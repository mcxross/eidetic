use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs, Wrap},
};

use super::app::{App, ConfirmAction, DetailView, Tab};
use crate::memory::types::*;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tabs + header
            Constraint::Min(0),    // main content
            Constraint::Length(1), // status bar
        ])
        .split(f.area());

    draw_header(f, app, chunks[0]);

    if app.confirm_action.is_some() {
        draw_confirm_dialog(f, app, chunks[1]);
    } else {
        match &app.detail {
            DetailView::None => draw_tab_content(f, app, chunks[1]),
            DetailView::ObservationDetail(_) => draw_observation_detail(f, app, chunks[1]),
            DetailView::SessionDetail(_) => draw_session_detail(f, app, chunks[1]),
        }
    }

    draw_status_bar(f, app, chunks[2]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = [
        Tab::Projects,
        Tab::Observations,
        Tab::Sessions,
        Tab::Search,
        Tab::Config,
    ]
    .iter()
    .map(|t| {
        let style = if *t == app.tab {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        Line::from(Span::styled(t.label(), style))
    })
    .collect();

    let project_name = app
        .active_project
        .as_ref()
        .map(|p| p.name.as_str())
        .unwrap_or("(none)");

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Eidetic TUI — Project: {} ", project_name)),
        )
        .select(match app.tab {
            Tab::Projects => 0,
            Tab::Observations => 1,
            Tab::Sessions => 2,
            Tab::Search => 3,
            Tab::Config => 4,
        })
        .highlight_style(Style::default().fg(Color::Cyan));

    f.render_widget(tabs, area);
}

fn draw_tab_content(f: &mut Frame, app: &App, area: Rect) {
    match app.tab {
        Tab::Projects => draw_projects(f, app, area),
        Tab::Observations => draw_observations(f, app, area),
        Tab::Sessions => draw_sessions(f, app, area),
        Tab::Search => draw_search(f, app, area),
        Tab::Config => draw_config(f, app, area),
    }
}

fn draw_projects(f: &mut Frame, app: &App, area: Rect) {
    if app.storage.as_structured().is_none() {
        let p = Paragraph::new("Semantic mode active.\n\nProjects, Observations, and Sessions are disabled in native Memwal mode.\nUse the Search tab to query the network.")
            .block(Block::default().borders(Borders::ALL).title(" Projects [Disabled] "));
        f.render_widget(p, area);
        return;
    }

    let header = Row::new(vec!["Name", "Path", "Created"]).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = app
        .projects
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let style = if i == app.project_cursor {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default()
            };
            Row::new(vec![
                Cell::from(p.name.clone()),
                Cell::from(p.path.clone()),
                Cell::from(p.created_at.format("%Y-%m-%d").to_string()),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Percentage(50),
            Constraint::Percentage(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Projects [Enter=Select] "),
    );

    f.render_widget(table, area);
}

fn draw_observations(f: &mut Frame, app: &App, area: Rect) {
    if app.storage.as_structured().is_none() {
        let p = Paragraph::new("Semantic mode active.\n\nProjects, Observations, and Sessions are disabled in native Memwal mode.\nUse the Search tab to query the network.")
            .block(Block::default().borders(Borders::ALL).title(" Observations [Disabled] "));
        f.render_widget(p, area);
        return;
    }

    let header = Row::new(vec!["Type", "Scope", "Title", "State", "Updated", "Topic"]).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = app
        .observations
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let style = if i == app.observation_cursor {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default()
            };
            let state_color = lifecycle_color(o.lifecycle);
            Row::new(vec![
                Cell::from(format!("{:?}", o.memory_type)),
                Cell::from(format!("{:?}", o.scope)),
                Cell::from(truncate(&o.title, 40)),
                Cell::from(Span::styled(
                    format!("{:?}", o.lifecycle),
                    Style::default().fg(state_color),
                )),
                Cell::from(o.updated_at.format("%m-%d %H:%M").to_string()),
                Cell::from(o.topic_key.clone().unwrap_or_default()),
            ])
            .style(style)
        })
        .collect();

    let page_info = format!(" Page {} ", app.observation_page + 1);
    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Min(20),
            Constraint::Length(10),
            Constraint::Length(14),
            Constraint::Length(16),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(format!(
        " Observations [d=Del r=Review Enter=Detail]{} ",
        page_info
    )));

    f.render_widget(table, area);
}

fn draw_sessions(f: &mut Frame, app: &App, area: Rect) {
    if app.storage.as_structured().is_none() {
        let p = Paragraph::new("Semantic mode active.\n\nProjects, Observations, and Sessions are disabled in native Memwal mode.\nUse the Search tab to query the network.")
            .block(Block::default().borders(Borders::ALL).title(" Sessions [Disabled] "));
        f.render_widget(p, area);
        return;
    }

    let header = Row::new(vec!["ID", "Started", "Ended", "Summary"]).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = app
        .sessions
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let style = if i == app.session_cursor {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default()
            };
            let ended = s
                .ended_at
                .map(|t| t.format("%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "active".to_string());
            let summary = s
                .summary
                .as_ref()
                .map(|sum| truncate(&sum.goal, 50))
                .unwrap_or_default();
            Row::new(vec![
                Cell::from(truncate(&s.id, 12)),
                Cell::from(s.started_at.format("%m-%d %H:%M").to_string()),
                Cell::from(ended),
                Cell::from(summary),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Min(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Sessions [Enter=Detail] "),
    );

    f.render_widget(table, area);
}

fn draw_search(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let input_style = if app.input_mode {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let input = Paragraph::new(app.input_buffer.as_str())
        .style(input_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Search [/=focus Enter=search Esc=unfocus] "),
        );
    f.render_widget(input, chunks[0]);

    let header = Row::new(vec!["Score", "Type", "Title", "Content"]).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = app
        .search_results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let style = if i == app.search_cursor {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default()
            };
            Row::new(vec![
                Cell::from(format!("{:.1}", r.score)),
                Cell::from(format!("{:?}", r.observation.memory_type)),
                Cell::from(truncate(&r.observation.title, 30)),
                Cell::from(truncate(&r.observation.content, 60)),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Min(20),
            Constraint::Min(30),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Results "));

    f.render_widget(table, chunks[1]);
}

fn draw_observation_detail(f: &mut Frame, app: &App, area: Rect) {
    let obs = match &app.detail_observation {
        Some(o) => o,
        None => {
            let p = Paragraph::new("Loading...").block(Block::default().borders(Borders::ALL));
            f.render_widget(p, area);
            return;
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);

    let content = Paragraph::new(obs.content.as_str())
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} ", obs.title)),
        );
    f.render_widget(content, chunks[0]);

    let meta_lines = vec![
        Line::from(vec![
            Span::styled("ID: ", Style::default().fg(Color::Yellow)),
            Span::raw(&obs.id),
        ]),
        Line::from(vec![
            Span::styled("Type: ", Style::default().fg(Color::Yellow)),
            Span::raw(format!("{:?}", obs.memory_type)),
        ]),
        Line::from(vec![
            Span::styled("Scope: ", Style::default().fg(Color::Yellow)),
            Span::raw(format!("{:?}", obs.scope)),
        ]),
        Line::from(vec![
            Span::styled("State: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{:?}", obs.lifecycle),
                Style::default().fg(lifecycle_color(obs.lifecycle)),
            ),
        ]),
        Line::from(vec![
            Span::styled("Hash: ", Style::default().fg(Color::Yellow)),
            Span::raw(truncate(&obs.hash, 16)),
        ]),
        Line::from(vec![
            Span::styled("Revisions: ", Style::default().fg(Color::Yellow)),
            Span::raw(obs.revision_count.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Duplicates: ", Style::default().fg(Color::Yellow)),
            Span::raw(obs.duplicate_count.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Created: ", Style::default().fg(Color::Yellow)),
            Span::raw(obs.created_at.format("%Y-%m-%d %H:%M").to_string()),
        ]),
        Line::from(vec![
            Span::styled("Updated: ", Style::default().fg(Color::Yellow)),
            Span::raw(obs.updated_at.format("%Y-%m-%d %H:%M").to_string()),
        ]),
        Line::from(vec![
            Span::styled("Last Seen: ", Style::default().fg(Color::Yellow)),
            Span::raw(obs.last_seen_at.format("%Y-%m-%d %H:%M").to_string()),
        ]),
        Line::from(vec![
            Span::styled("Review After: ", Style::default().fg(Color::Yellow)),
            Span::raw(
                obs.review_after
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| "—".to_string()),
            ),
        ]),
        Line::from(vec![
            Span::styled("Topic: ", Style::default().fg(Color::Yellow)),
            Span::raw(obs.topic_key.clone().unwrap_or_else(|| "—".to_string())),
        ]),
        Line::from(vec![
            Span::styled("Tags: ", Style::default().fg(Color::Yellow)),
            Span::raw(if obs.tags.is_empty() {
                "—".to_string()
            } else {
                obs.tags.join(", ")
            }),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "[Esc] Back",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let meta = Paragraph::new(meta_lines)
        .block(Block::default().borders(Borders::ALL).title(" Metadata "));
    f.render_widget(meta, chunks[1]);
}

fn draw_session_detail(f: &mut Frame, app: &App, area: Rect) {
    let sess = match &app.detail_session {
        Some(s) => s,
        None => {
            let p = Paragraph::new("Loading...").block(Block::default().borders(Borders::ALL));
            f.render_widget(p, area);
            return;
        }
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(0)])
        .split(area);

    let ended = sess
        .ended_at
        .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "active".to_string());
    let summary_text = sess
        .summary
        .as_ref()
        .map(|s| {
            format!(
                "Goal: {}\nDiscoveries: {}\nAccomplished: {}\nNext: {}",
                s.goal,
                s.discoveries.join(", "),
                s.accomplished.join(", "),
                s.next_steps.join(", "),
            )
        })
        .unwrap_or_else(|| "No summary".to_string());

    let info = Paragraph::new(format!(
        "ID: {}\nStarted: {}\nEnded: {}\n{}",
        sess.id,
        sess.started_at.format("%Y-%m-%d %H:%M"),
        ended,
        summary_text,
    ))
    .wrap(Wrap { trim: false })
    .block(Block::default().borders(Borders::ALL).title(" Session "));
    f.render_widget(info, chunks[0]);

    let header = Row::new(vec!["Type", "Title", "Created"]).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = app
        .detail_session_observations
        .iter()
        .map(|o| {
            Row::new(vec![
                Cell::from(format!("{:?}", o.memory_type)),
                Cell::from(truncate(&o.title, 50)),
                Cell::from(o.created_at.format("%m-%d %H:%M").to_string()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Min(30),
            Constraint::Length(14),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(format!(
        " Observations ({}) [Esc=Back] ",
        app.detail_session_observations.len()
    )));
    f.render_widget(table, chunks[1]);
}

fn draw_confirm_dialog(f: &mut Frame, app: &App, area: Rect) {
    let msg = match &app.confirm_action {
        Some(ConfirmAction::HardDelete(id)) => format!(
            "Hard-delete observation {}?\n\nThis cannot be undone.\n\n[y] Yes  [n/Esc] No",
            truncate(id, 20)
        ),
        None => return,
    };

    let dialog_area = centered_rect(50, 30, area);
    let dialog = Paragraph::new(msg)
        .style(Style::default().fg(Color::Red))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" ⚠ Confirm ")
                .style(Style::default().fg(Color::Red)),
        );
    f.render_widget(dialog, dialog_area);
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let keybinds = match app.detail {
        DetailView::None => {
            "q:Quit  Tab:Switch  ↑↓:Nav  Enter:Select  /:Search  d:Del  D:HardDel  r:Review  n/p:Page"
        }
        _ => "Esc:Back  q:Quit",
    };
    let status_text = if app.status.is_empty() {
        keybinds.to_string()
    } else {
        format!("{} │ {}", app.status, keybinds)
    };

    let bar = Paragraph::new(status_text).style(Style::default().fg(Color::DarkGray));
    f.render_widget(bar, area);
}

fn lifecycle_color(state: LifecycleState) -> Color {
    match state {
        LifecycleState::Active => Color::Green,
        LifecycleState::Review => Color::Yellow,
        LifecycleState::Archived => Color::DarkGray,
        LifecycleState::Deleted => Color::Red,
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_config(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec!["Key", "Value"]).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let mut rows: Vec<Row> = Vec::new();

    let storage_backend = app
        .config
        .storage_backend
        .clone()
        .unwrap_or_else(|| String::from("memwal (default)"));
    rows.push(Row::new(vec![
        Cell::from("storage_backend"),
        Cell::from(storage_backend),
    ]));

    let storage_path = app
        .config
        .storage_path
        .clone()
        .unwrap_or_else(|| String::from("(default)"));
    rows.push(Row::new(vec![
        Cell::from("storage_path"),
        Cell::from(storage_path),
    ]));

    let sui_config_dir = app
        .config
        .sui_config_dir
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| String::from("(default)"));
    rows.push(Row::new(vec![
        Cell::from("sui_config_dir"),
        Cell::from(sui_config_dir),
    ]));

    let memwal_account_id = app
        .config
        .memwal_account_id
        .clone()
        .unwrap_or_else(|| String::from("(none)"));
    rows.push(Row::new(vec![
        Cell::from("memwal_account_id"),
        Cell::from(memwal_account_id),
    ]));

    let memwal_registry_id = app
        .config
        .memwal_registry_id
        .clone()
        .unwrap_or_else(|| String::from("(none)"));
    rows.push(Row::new(vec![
        Cell::from("memwal_registry_id"),
        Cell::from(memwal_registry_id),
    ]));

    let memwal_server_url = app
        .config
        .memwal_server_url
        .clone()
        .unwrap_or_else(|| String::from("(none)"));
    rows.push(Row::new(vec![
        Cell::from("memwal_server_url"),
        Cell::from(memwal_server_url),
    ]));

    let memwal_relayer_config_url = app
        .config
        .memwal_relayer_config_url
        .clone()
        .or_else(|| {
            app.config
                .memwal_server_url
                .as_ref()
                .map(|url| format!("{}/config (default)", url))
        })
        .unwrap_or_else(|| String::from("(none)"));
    rows.push(Row::new(vec![
        Cell::from("memwal_relayer_config_url"),
        Cell::from(memwal_relayer_config_url),
    ]));

    let memwal_namespace = app
        .config
        .memwal_namespace
        .clone()
        .unwrap_or_else(|| String::from("eidetic (default)"));
    rows.push(Row::new(vec![
        Cell::from("memwal_namespace"),
        Cell::from(memwal_namespace),
    ]));

    let memwal_delegate_label = app
        .config
        .memwal_delegate_label
        .clone()
        .unwrap_or_else(|| String::from("(generated at setup)"));
    rows.push(Row::new(vec![
        Cell::from("memwal_delegate_label"),
        Cell::from(memwal_delegate_label),
    ]));

    if crate::auth::KeychainManager::is_configured() {
        rows.push(Row::new(vec![
            Cell::from("private_key"),
            Cell::from("********"),
        ]));
    } else {
        rows.push(Row::new(vec![
            Cell::from("private_key"),
            Cell::from("(none)"),
        ]));
    }

    if crate::harbor::HarborCredentials::is_configured() {
        rows.push(Row::new(vec![
            Cell::from("harbor.api_key"),
            Cell::from("********"),
        ]));
    } else {
        rows.push(Row::new(vec![
            Cell::from("harbor.api_key"),
            Cell::from("(none)"),
        ]));
    }

    if let Some(ref harbor) = app.config.harbor {
        let bucket_id = harbor
            .bucket_id
            .clone()
            .unwrap_or_else(|| String::from("(none)"));
        rows.push(Row::new(vec![
            Cell::from("harbor.bucket_id"),
            Cell::from(bucket_id),
        ]));

        let seal_policy_id = harbor
            .seal_policy_id
            .clone()
            .unwrap_or_else(|| String::from("(none)"));
        rows.push(Row::new(vec![
            Cell::from("harbor.seal_policy_id"),
            Cell::from(seal_policy_id),
        ]));

        let last_backup = harbor
            .last_backup_at
            .clone()
            .unwrap_or_else(|| String::from("(never)"));
        rows.push(Row::new(vec![
            Cell::from("harbor.last_backup_at"),
            Cell::from(last_backup),
        ]));
    }

    let table = Table::new(
        rows,
        [Constraint::Percentage(30), Constraint::Percentage(70)],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Eidetic Configuration "),
    )
    .row_highlight_style(Style::default().bg(Color::Cyan).fg(Color::Black));

    f.render_widget(table, area);
}
