use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;
use tokio::sync::mpsc;

pub enum AppEvent {
    Key(KeyEvent),
    Tick,
}

pub fn spawn_event_loop(tick_rate: Duration) -> mpsc::UnboundedReceiver<AppEvent> {
    let (tx, rx) = mpsc::unbounded_channel();

    std::thread::spawn(move || {
        loop {
            if event::poll(tick_rate).unwrap_or(false) {
                if let Ok(Event::Key(key)) = event::read() {
                    if tx.send(AppEvent::Key(key)).is_err() {
                        return; // receiver dropped — app exiting
                    }
                }
            } else if tx.send(AppEvent::Tick).is_err() {
                return;
            }
        }
    });

    rx
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    Back,
    NavigateUp,
    NavigateDown,
    NextPage,
    PrevPage,
    Enter,
    Delete,
    HardDelete,
    MarkReviewed,
    Edit,
    Search,
    SwitchTab,
    InputChar(char),
    InputBackspace,
    InputConfirm,
}

pub fn map_key(key: KeyEvent, input_mode: bool) -> Action {
    if input_mode {
        match key.code {
            KeyCode::Esc => Action::Back,
            KeyCode::Enter => Action::InputConfirm,
            KeyCode::Backspace => Action::InputBackspace,
            KeyCode::Char(c) => Action::InputChar(c),
            _ => Action::None,
        }
    } else {
        match key.code {
            KeyCode::Char('q') => Action::Quit,
            KeyCode::Esc => Action::Back,
            KeyCode::Char('j') | KeyCode::Down => Action::NavigateDown,
            KeyCode::Char('k') | KeyCode::Up => Action::NavigateUp,
            KeyCode::Char('n') => Action::NextPage,
            KeyCode::Char('p') => Action::PrevPage,
            KeyCode::Enter => Action::Enter,
            KeyCode::Char('d') => Action::Delete,
            KeyCode::Char('D') => Action::HardDelete,
            KeyCode::Char('r') => Action::MarkReviewed,
            KeyCode::Char('e') => Action::Edit,
            KeyCode::Char('/') => Action::Search,
            KeyCode::Tab => Action::SwitchTab,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::Quit,
            _ => Action::None,
        }
    }
}
