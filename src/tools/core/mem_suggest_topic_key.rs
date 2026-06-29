use crate::memory::types::*;
use crate::storage::MemoryStore;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemSuggestTopicKeyParams {
    #[schemars(description = "Project ID (optional, will auto-detect from cwd if not provided)")]
    pub project_id: Option<String>,
    #[schemars(description = "Content to analyze for topic suggestion")]
    pub content: String,
    #[schemars(description = "Optional existing topic key to compare against")]
    pub existing_topic_key: Option<String>,
}

#[derive(Clone)]
pub struct MemSuggestTopicKey {
    store: MemoryStore,
}

impl MemSuggestTopicKey {
    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }

    #[tool(description = "Suggest a stable topic_key for evolving topics before saving")]
    pub async fn mem_suggest_topic_key(
        &self,
        Parameters(params): Parameters<MemSuggestTopicKeyParams>,
    ) -> Result<CallToolResult, McpError> {
        if params.content.trim().is_empty() {
            return Err(McpError::invalid_params("content must not be empty", None));
        }
        let storage = self.store.storage();
        let structured = match storage.as_structured() {
            Some(s) => s,
            None => return Err(McpError::internal_error("mem_suggest_topic_key is not supported on unstructured storage backends like memwal", None)),
        };

        let project = if let Some(pid) = params.project_id {
            structured
                .get_project(&pid)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
                .ok_or_else(|| {
                    McpError::invalid_params(format!("Project not found: {}", pid), None)
                })?
        } else {
            self.store
                .get_or_create_project(None)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        };

        let project_id = project.id.clone();

        let observations = structured
            .list_observations(&project_id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let existing_topics: Vec<TopicKey> = observations
            .iter()
            .filter_map(|o| o.topic_key.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        let suggested_key = self.extract_topic_key(&params.content);

        let mut similar = Vec::new();
        for topic in &existing_topics {
            let similarity = self.calculate_similarity(&suggested_key, topic);
            if similarity > 0.3 {
                similar.push((topic.clone(), similarity));
            }
        }

        similar.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let similar_topics: Vec<TopicKey> = similar.into_iter().take(5).map(|(t, _)| t.to_string()).collect();

        let confidence = if similar_topics.is_empty() { 0.8 } else { 0.6 };

        let suggestion = TopicSuggestion {
            suggested_key: suggested_key.clone(),
            confidence,
            existing_similar: similar_topics,
            reasoning: format!(
                "Extracted key terms from content: '{}'. Similar existing topics: {}",
                self.extract_key_terms(&params.content).join(", "),
                if existing_topics.is_empty() {
                    "none".to_string()
                } else {
                    existing_topics.join(", ")
                }
            ),
        };

        let output = format!(
            "Suggested topic_key: {}\nConfidence: {:.0}%\nSimilar existing: {}\nReasoning: {}",
            suggestion.suggested_key,
            suggestion.confidence * 100.0,
            if suggestion.existing_similar.is_empty() {
                "none".to_string()
            } else {
                suggestion.existing_similar.join(", ")
            },
            suggestion.reasoning
        );

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    fn extract_topic_key(&self, content: &str) -> TopicKey {
        let terms = self.extract_key_terms(content);
        if terms.is_empty() {
            "general".to_string()
        } else {
            terms.into_iter().take(3).collect::<Vec<_>>().join("-")
        }
    }

    fn extract_key_terms(&self, content: &str) -> Vec<String> {
        let stop_words: std::collections::HashSet<&str> = [
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with",
            "by", "from", "as", "is", "was", "are", "were", "been", "be", "have", "has", "had",
            "do", "does", "did", "will", "would", "could", "should", "may", "might", "must",
            "this", "that", "these", "those", "i", "you", "he", "she", "it", "we", "they", "my",
            "your", "his", "her", "its", "our", "their", "me", "him", "us", "them",
        ]
        .into_iter()
        .collect();

        content
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 2 && !stop_words.contains(*w))
            .map(|w| w.to_string())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .take(10)
            .collect()
    }

    fn calculate_similarity(&self, a: &str, b: &str) -> f32 {
        let a_terms: std::collections::HashSet<_> = a.split('-').collect();
        let b_terms: std::collections::HashSet<_> = b.split('-').collect();

        let intersection = a_terms.intersection(&b_terms).count();
        let union = a_terms.union(&b_terms).count();

        if union == 0 {
            0.0
        } else {
            intersection as f32 / union as f32
        }
    }
}
