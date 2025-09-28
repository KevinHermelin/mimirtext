use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    model::{
        Message, Model, NotePaneMessage, NotePaneModel, SearchWindowMessage, SearchWindowModel,
    },
    text_input::InputOperation,
};

pub trait KeyHandler<Message> {
    fn handle_key_event(&self, key_event: KeyEvent) -> Message;
}

impl KeyHandler<Message> for Model {
    fn handle_key_event(&self, key_event: KeyEvent) -> Message {
        if let Some(search_window) = &self.search_window {
            if let KeyCode::Esc = key_event.code {
                return Message::CloseSearch;
            }
            return Message::SearchWindow(search_window.handle_key_event(key_event));
        }

        let message = match (key_event.code, key_event.modifiers) {
            (KeyCode::Char('q'), _) => Message::Quit,
            (KeyCode::Char('p'), KeyModifiers::CONTROL) => Message::OpenSearch,
            _ => Message::None,
        };

        if !matches!(message, Message::None) {
            return message;
        }

        return Message::NotePane(self.note_pane.handle_key_event(key_event));
    }
}

impl KeyHandler<NotePaneMessage> for NotePaneModel {
    fn handle_key_event(&self, key_event: KeyEvent) -> NotePaneMessage {
        match (key_event.code, key_event.modifiers) {
            (KeyCode::Down, _) => NotePaneMessage::ScrollDown,
            (KeyCode::Up, _) => NotePaneMessage::ScrollUp,
            (KeyCode::Right, _) => NotePaneMessage::NextLink,
            (KeyCode::Left, _) => NotePaneMessage::PreviousLink,
            (KeyCode::Enter, _) => NotePaneMessage::FollowLink,
            (KeyCode::Backspace, _) => NotePaneMessage::PopNote,
            (KeyCode::Esc, _) => NotePaneMessage::StopEdit,
            (KeyCode::Char('c'), KeyModifiers::NONE) => NotePaneMessage::StartEdit,
            (KeyCode::Char('C'), KeyModifiers::SHIFT) => NotePaneMessage::EditExternally,
            _ => NotePaneMessage::None,
        }
    }
}

impl KeyHandler<SearchWindowMessage> for SearchWindowModel {
    fn handle_key_event(&self, key_event: KeyEvent) -> SearchWindowMessage {
        match (key_event.code, key_event.modifiers) {
            (KeyCode::Backspace, _) => SearchWindowMessage::Input(InputOperation::Backspace),
            (KeyCode::Enter, _) => SearchWindowMessage::OpenResult,
            (KeyCode::Left, _) => SearchWindowMessage::Input(InputOperation::Left),
            (KeyCode::Right, _) => SearchWindowMessage::Input(InputOperation::Right),
            (KeyCode::Down, _) => SearchWindowMessage::NextResult,
            (KeyCode::Up, _) => SearchWindowMessage::PreviousResult,
            (KeyCode::Char(c), _) => {
                SearchWindowMessage::Input(InputOperation::Insert(c.to_string()))
            }
            _ => SearchWindowMessage::None,
        }
    }
}
