use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    model::{
        EditState, Message, Model, NotePaneMessage, NotePaneModel, NotePaneState,
        SearchWindowMessage, SearchWindowModel,
    },
    text_input::{InputOperation, TextInput},
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
        // Common for all modes.
        match (key_event.code, key_event.modifiers) {
            (KeyCode::Esc, _) => return NotePaneMessage::StopEdit,
            _ => {}
        };

        if let NotePaneState::WithNote(context) = &self.state {
            if let EditState::Active(editor) = &context.editor {
                return NotePaneMessage::Input(editor.handle_key_event(key_event));
            }
        }

        match (key_event.code, key_event.modifiers) {
            (KeyCode::Down, _) => NotePaneMessage::ScrollDown,
            (KeyCode::Up, _) => NotePaneMessage::ScrollUp,
            (KeyCode::Right, _) => NotePaneMessage::NextLink,
            (KeyCode::Left, _) => NotePaneMessage::PreviousLink,
            (KeyCode::Enter, _) => NotePaneMessage::FollowLink,
            (KeyCode::Backspace, _) => NotePaneMessage::PopNote,
            (KeyCode::Char('c'), KeyModifiers::NONE) => NotePaneMessage::StartEdit,
            (KeyCode::Char('C'), KeyModifiers::SHIFT) => NotePaneMessage::EditExternally,
            _ => NotePaneMessage::None,
        }
    }
}

impl KeyHandler<SearchWindowMessage> for SearchWindowModel {
    fn handle_key_event(&self, key_event: KeyEvent) -> SearchWindowMessage {
        match (key_event.code, key_event.modifiers) {
            (KeyCode::Enter, _) => SearchWindowMessage::OpenResult,
            (KeyCode::Down, _) => SearchWindowMessage::NextResult,
            (KeyCode::Up, _) => SearchWindowMessage::PreviousResult,
            _ => SearchWindowMessage::Input(self.input.handle_key_event(key_event)),
        }
    }
}

impl KeyHandler<InputOperation> for TextInput {
    fn handle_key_event(&self, key_event: KeyEvent) -> InputOperation {
        match (key_event.code, key_event.modifiers) {
            (KeyCode::Backspace, _) => InputOperation::Backspace,
            (KeyCode::Left, _) => InputOperation::Left,
            (KeyCode::Right, _) => InputOperation::Right,
            (KeyCode::Up, _) => InputOperation::Up,
            (KeyCode::Down, _) => InputOperation::Down,
            (KeyCode::Char(c), _) => InputOperation::Insert(c.to_string()),
            _ => InputOperation::None,
        }
    }
}
