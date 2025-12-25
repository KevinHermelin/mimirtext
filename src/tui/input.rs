use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    model::{
        Message, Model,
        note_pane::{NotePaneMessage, NotePaneModel, NotePaneState},
        search_window::{SearchWindowMessage, SearchWindowModel},
    },
    text_input::{InputOperation, TextInput, Unit},
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
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => Message::Quit,
            (KeyCode::Char('p'), KeyModifiers::CONTROL) => Message::OpenSearch,
            _ => Message::None,
        };

        if !matches!(message, Message::None) {
            return message;
        }

        Message::NotePane(self.note_pane.handle_key_event(key_event))
    }
}

impl KeyHandler<NotePaneMessage> for NotePaneModel {
    fn handle_key_event(&self, key_event: KeyEvent) -> NotePaneMessage {
        // Common for all modes.
        if let (KeyCode::Esc, _) = (key_event.code, key_event.modifiers) {
            return NotePaneMessage::StopEdit;
        };

        if let NotePaneState::With(context) = &self.state {
            if let Some(editor) = &context.editor {
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
            (KeyCode::Char('j'), KeyModifiers::CONTROL) => SearchWindowMessage::CreateNew,
            (KeyCode::Enter, KeyModifiers::NONE) => SearchWindowMessage::OpenResult,
            (KeyCode::Down, _) => SearchWindowMessage::NextResult,
            (KeyCode::Up, _) => SearchWindowMessage::PreviousResult,
            _ => SearchWindowMessage::Input(self.input.handle_key_event(key_event)),
        }
    }
}

impl KeyHandler<InputOperation> for TextInput {
    fn handle_key_event(&self, key_event: KeyEvent) -> InputOperation {
        if self.completions().is_some() {
            match key_event.code {
                KeyCode::Enter => return InputOperation::Complete,
                KeyCode::Up => return InputOperation::PreviousCompletion,
                KeyCode::Down => return InputOperation::NextCompletion,
                _ => {}
            };
        }

        match (key_event.code, key_event.modifiers) {
            (KeyCode::Backspace, _) => InputOperation::Backspace,
            (KeyCode::Left, KeyModifiers::NONE) => InputOperation::Left(Unit::Char),
            (KeyCode::Left, KeyModifiers::CONTROL) => InputOperation::Left(Unit::Word),
            (KeyCode::Right, KeyModifiers::NONE) => InputOperation::Right(Unit::Char),
            (KeyCode::Right, KeyModifiers::CONTROL) => InputOperation::Right(Unit::Word),
            (KeyCode::Up, _) => InputOperation::Up,
            (KeyCode::Down, _) => InputOperation::Down,
            (KeyCode::Enter, _) => InputOperation::Insert(String::from("\n")),
            (KeyCode::Tab, _) => InputOperation::Insert(String::from("\t")),
            (KeyCode::Char(c), _) => InputOperation::Insert(c.to_string()),
            _ => InputOperation::None,
        }
    }
}
