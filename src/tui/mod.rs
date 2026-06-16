pub mod app;
pub mod events;
pub mod ui;

use std::io;
use std::sync::Arc;
use std::time::Duration;

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::storage::Storage;
use app::{App, ConfirmAction, DetailView, Tab};
use events::{map_key, spawn_event_loop, Action, AppEvent};

pub async fn run(storage: Arc<dyn Storage>) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(storage);

    app.load_projects().await?;

    let mut events = spawn_event_loop(Duration::from_millis(250));

    while app.running {
        terminal.draw(|f| ui::draw(f, &app))?;

        if let Some(event) = events.recv().await {
            match event {
                AppEvent::Key(key) => {
                    if let Some(ref confirm) = app.confirm_action.clone() {
                        match key.code {
                            crossterm::event::KeyCode::Char('y') => {
                                match confirm {
                                    ConfirmAction::HardDelete(id) => {
                                        let id = id.clone();
                                        let _ = app.hard_delete_confirmed(&id).await;
                                    }
                                }
                            }
                            _ => {
                                app.confirm_action = None;
                            }
                        }
                        continue;
                    }

                    let action = map_key(key, app.input_mode);
                    match action {
                        Action::Quit => app.running = false,
                        Action::Back => {
                            if app.input_mode {
                                app.input_mode = false;
                            } else if app.detail != DetailView::None {
                                app.detail = DetailView::None;
                            } else if app.active_project.is_some() && app.tab != Tab::Projects {
                                app.tab = Tab::Projects;
                            } else {
                                app.running = false;
                            }
                        }
                        Action::NavigateUp => app.cursor_up(),
                        Action::NavigateDown => app.cursor_down(),
                        Action::SwitchTab => {
                            app.tab = app.tab.next();
                            app.detail = DetailView::None;
                            let _ = app.reload_current_list().await;
                        }
                        Action::NextPage => {
                            match app.tab {
                                Tab::Observations => {
                                    app.observation_page += 1;
                                    let _ = app.load_observations().await;
                                    if app.observations.is_empty() && app.observation_page > 0 {
                                        app.observation_page -= 1;
                                        let _ = app.load_observations().await;
                                    }
                                }
                                Tab::Sessions => {
                                    app.session_page += 1;
                                    let _ = app.load_sessions().await;
                                    if app.sessions.is_empty() && app.session_page > 0 {
                                        app.session_page -= 1;
                                        let _ = app.load_sessions().await;
                                    }
                                }
                                _ => {}
                            }
                        }
                        Action::PrevPage => {
                            match app.tab {
                                Tab::Observations => {
                                    if app.observation_page > 0 {
                                        app.observation_page -= 1;
                                        let _ = app.load_observations().await;
                                    }
                                }
                                Tab::Sessions => {
                                    if app.session_page > 0 {
                                        app.session_page -= 1;
                                        let _ = app.load_sessions().await;
                                    }
                                }
                                _ => {}
                            }
                        }
                        Action::Enter => {
                            if app.detail != DetailView::None {
                            } else {
                                match app.tab {
                                    Tab::Projects => { let _ = app.enter_project().await; }
                                    Tab::Observations => { let _ = app.enter_observation_detail().await; }
                                    Tab::Sessions => { let _ = app.enter_session_detail().await; }
                                    Tab::Search => { let _ = app.enter_observation_detail().await; }
                                }
                            }
                        }
                        Action::Delete => {
                            let _ = app.soft_delete_selected().await;
                        }
                        Action::HardDelete => {
                            if let Some(obs) = app.selected_observation() {
                                app.confirm_action = Some(ConfirmAction::HardDelete(obs.id.clone()));
                            }
                        }
                        Action::MarkReviewed => {
                            let _ = app.mark_reviewed_selected().await;
                        }
                        Action::Search => {
                            app.tab = Tab::Search;
                            app.input_mode = true;
                            app.detail = DetailView::None;
                        }
                        Action::InputChar(c) => {
                            app.input_buffer.push(c);
                        }
                        Action::InputBackspace => {
                            app.input_buffer.pop();
                        }
                        Action::InputConfirm => {
                            app.input_mode = false;
                            let _ = app.do_search().await;
                        }
                        Action::Edit | Action::None => {}
                    }
                }
                AppEvent::Tick => {
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
