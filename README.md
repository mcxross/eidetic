# Eidetic

**Eidetic** is an Agent-Agnostic Memory MCP Server. It provides a structured, long-term memory system via the Model Context Protocol (MCP).

Eidetic enables **memory portability**: a memory created by one agent (e.g. Claude Code) can be instantly recalled by another (e.g. Cursor). 

Agents can automatically store, recall, deduplicate, and review project knowledge—such as architectural decisions, user preferences, and bug resolutions—so they never have to start from scratch.

---

## Index

- [Installation](#installation)
- [Agent Integration](#agent-integration)
- [Features](#features)
- [Storage Backends](#storage-backends)
  - [Memwal](#memwal)
  - [SQLite](#sqlite)
  - [File](#file)
- [MCP Tools](#mcp-tools)
- [TUI Storage Inspector](#tui-storage-inspector)
- [License](#license)

---

## Installation

You can install Eidetic in two ways:

### 1. Via NPM (Recommended)
You can install the pre-compiled binary wrapper globally using npm:

```bash
npm install -g eidetic-mcp
```

Or run it directly using `npx`:
```bash
npx eidetic-mcp
```

### 2. Via Cargo (From Source)
If you prefer to build from source, you can use Cargo:

```bash
git clone https://github.com/mcxross/eidetic.git
cd eidetic
cargo install --path .
```

---

## Agent Integration

Eidetic features a built-in auto-setup command to easily inject itself into the configuration files of popular coding agents.

Run the following command to register Eidetic with your agent of choice:

```bash
eidetic setup <agent>
```

### Supported Agents
*   `claude` - Claude Code CLI
*   `claude-desktop` - Claude Desktop App
*   `cursor` (or `pi`) - Cursor IDE
*   `vscode` - Visual Studio Code
*   `opencode` - OpenCode
*   `gemini-cli` - Gemini CLI
*   `codex` - OpenAI Codex CLI

**Example**:
```bash
eidetic setup cursor
```
This will automatically parse your `~/.cursor/mcp.json` file, safely merge Eidetic as an available MCP server, and point the agent to the exact Eidetic executable path.

---

## Features

### MCP Server
Start the MCP server manually (usually handled automatically by your agent):
```bash
eidetic serve
```
By default, the server uses a SQLite database to store memories locally.

---

## Storage Backends

Eidetic supports multiple storage layers through `--storage-backend` or the `EIDETIC_STORAGE_BACKEND` environment variable.

Available backends:

| Backend | Use case | Persistence |
| --- | --- | --- |
| `memwal` | Semantic memory backed by Memwal, with local SQLite exact CRUD index | Memwal remote storage plus local SQLite index |
| `sqlite` | Default local storage | Local SQLite database |
| `file` | Simple JSON file storage | Local JSON files |

Use `--storage-path` or `EIDETIC_STORAGE_PATH` to choose the local storage/index directory.

### Memwal

Memwal is the recommended backend when you want Eidetic memories to be searchable through Memwal while preserving Eidetic's MCP behavior for exact reads, updates, deletes, projects, sessions, prompts, and relations.

Start the MCP server with Memwal:

```bash
eidetic --storage-backend memwal serve
```

Or configure it with environment variables:

```bash
export EIDETIC_STORAGE_BACKEND=memwal
export EIDETIC_MEMWAL_REGISTRY_ID=0x...
eidetic serve
```

Memwal account selection is handled by Eidetic's authentication layer:

1. On startup, Eidetic reads `~/.sui/sui_config/client.yaml`, `sui.aliases`, and the configured `sui.keystore`.
2. It derives available Ed25519 Sui accounts locally from private key material.
3. It defaults to `client.yaml.active_address` when that account has usable key material.
4. The memory storage layer receives only the active Memwal client/configuration; it does not manage private keys directly.

Memwal configuration options:

| CLI flag | Environment variable | Description |
| --- | --- | --- |
| `--memwal-account-id` | `EIDETIC_MEMWAL_ACCOUNT_ID` | Existing Memwal account object ID to reuse |
| `--memwal-registry-id` | `EIDETIC_MEMWAL_REGISTRY_ID` | Memwal registry object ID for account reuse/creation |
| `--memwal-server-url` | `EIDETIC_MEMWAL_SERVER_URL` | Memwal relayer/server URL |
| `--memwal-relayer-config-url` | `EIDETIC_MEMWAL_RELAYER_CONFIG_URL` | Explicit relayer config URL |
| `--memwal-namespace` | `EIDETIC_MEMWAL_NAMESPACE` | Memory namespace, defaults to `eidetic` |
| `--memwal-delegate-label` | `EIDETIC_MEMWAL_DELEGATE_LABEL` | Delegate key label, defaults to `eidetic-mcp` |
| `--sui-config-dir` | `EIDETIC_SUI_CONFIG_DIR` | Override Sui config directory, defaults to `~/.sui/sui_config` |

Memwal MCP utility tools:

| Tool | Purpose |
| --- | --- |
| `mem_sui_accounts` | Lists usable Sui accounts discovered from `~/.sui` without exposing private keys |
| `mem_select_sui_account` | Selects the account used by Memwal operations for the current server process |
| `mem_memwal_config` | Shows redacted active Memwal configuration and backend status |

Example account flow:

```text
mem_sui_accounts
mem_select_sui_account { "selector": "my-sui-alias-or-0x-address" }
mem_memwal_config
```

Account selection is process-local. Eidetic does not mutate `~/.sui/client.yaml` and does not persist the selected Memwal account across server restarts. If no account is selected after restart, Eidetic reloads `~/.sui` and falls back to Sui's `active_address`.

Memwal writes store a stable Eidetic payload in Memwal and also write a local SQLite index. Search first attempts Memwal recall, then falls back to the local SQLite index for exact matching and compatibility.

### SQLite

SQLite is the default backend and requires no external services:

```bash
eidetic --storage-backend sqlite serve
```

Choose a custom database directory:

```bash
eidetic --storage-backend sqlite --storage-path ~/.local/share/eidetic-mcp/storage serve
```

SQLite stores all projects, observations, sessions, prompts, relations, and search index data locally.

### File

The file backend stores JSON documents on disk:

```bash
eidetic --storage-backend file serve
```

Choose a custom file storage directory:

```bash
eidetic --storage-backend file --storage-path ~/.local/share/eidetic-mcp/files serve
```

Use this backend when you want easy inspection or simple portable JSON files. SQLite is generally a better default for normal use.

---

## MCP Tools

Eidetic exposes the following MCP tools to connected agents.

### Core Memory

| Tool | Purpose |
| --- | --- |
| `mem_save` | Save a structured observation such as a decision, bugfix, pattern, discovery, task, note, or learning |
| `mem_update` | Update an existing observation by ID |
| `mem_delete` | Delete an observation; soft delete is the default, hard delete is optional |
| `mem_search` | Full-text search across memories for the current or specified project |
| `mem_get_observation` | Fetch the full content and metadata for a specific memory |
| `mem_suggest_topic_key` | Suggest a stable `topic_key` before saving evolving topic-based memories |
| `mem_save_prompt` | Save a user prompt for future context |

### Sessions

| Tool | Purpose |
| --- | --- |
| `mem_session_start` | Register the start of a work session |
| `mem_session_end` | Mark a session as completed |
| `mem_context` | Retrieve recent context from previous sessions and observations |
| `mem_session_summary` | Save an end-of-session summary |

### Projects

| Tool | Purpose |
| --- | --- |
| `mem_current_project` | Detect the current project from a working directory; recommended as a first call |
| `mem_merge_projects` | Merge project name variants into a canonical project |

### Advanced Review and Relations

| Tool | Purpose |
| --- | --- |
| `mem_timeline` | Get chronological context around a specific observation |
| `mem_capture_passive` | Extract learnings from text output and save them as memories |
| `mem_review` | List observations whose review lifecycle is stale; can mark items reviewed |
| `mem_judge` | Record a verdict for a pending memory conflict surfaced by `mem_save` |
| `mem_compare` | Persist a semantic relation verdict between two existing observations |

### Diagnostics and Configuration

| Tool | Purpose |
| --- | --- |
| `mem_stats` | Return memory system statistics for a project |
| `mem_doctor` | Run read-only diagnostics for project detection and storage health |

### Memwal Account Tools

These tools are available when the server is running with `--storage-backend memwal`.

| Tool | Purpose |
| --- | --- |
| `mem_sui_accounts` | List usable Sui accounts discovered from `~/.sui` without exposing private keys |
| `mem_select_sui_account` | Select the Sui account used by Memwal operations for the current server process |
| `mem_memwal_config` | Show redacted active Memwal account and backend configuration |

### TUI Storage Inspector
Eidetic comes with a built-in Terminal UI to easily inspect, search, review, and delete the memories stored by your agents.

```bash
eidetic tui
```
**Controls:**
*   `Tab` - Switch views (Projects, Observations, Sessions, Search)
*   `Enter` - View details for an observation
*   `d` - Soft-delete an observation
*   `D` - Hard-delete an observation
*   `r` - Mark an observation as reviewed
*   `/` - Open search

---

## License
Apache 2.0
