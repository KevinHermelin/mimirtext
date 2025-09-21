use crate::{
    markdown_view::{LinkRef, LinkTarget, MarkdownDocument},
    repository::{NoteKey, NoteSnapshot},
};

pub trait ClampAdd: Sized {
    fn add_clamped(self, delta: isize, min: isize, max: isize) -> Self;
}

impl ClampAdd for isize {
    fn add_clamped(self, delta: isize, min: isize, max: isize) -> Self {
        (self + delta).clamp(min, max)
    }
}

pub trait Update<Message> {
    fn update(&self, message: Message) -> (Self, Command)
    where
        Self: Sized;
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum RunningState {
    #[default]
    Running,
    Done,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    pub running_state: RunningState,
    pub note_pane: NotePaneModel,
}

pub enum Message {
    Quit,
    NotePane(NotePaneMessage),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    None,
    FollowLink(LinkTarget),
    ServeNote(NoteKey),
    EditExternally(NoteKey),
}

impl Update<Message> for Model {
    fn update(&self, message: Message) -> (Model, Command) {
        let mut model = self.clone();
        let mut command = Command::None;

        match message {
            Message::Quit => model.running_state = RunningState::Done,
            Message::NotePane(message) => {
                (model.note_pane, command) = model.note_pane.update(message)
            }
        }
        (model, command)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NoteContext {
    note: NoteSnapshot,
    scroll_lines: isize,
    link_selection_index: isize,
}

impl NoteContext {
    fn document(&self) -> MarkdownDocument {
        MarkdownDocument::new(&self.note.body)
    }

    fn max_scroll(&self) -> isize {
        self.note.body.lines().count().saturating_sub(1) as isize
    }

    fn max_link_selection(&self) -> isize {
        self.document().get_links().iter().count().saturating_sub(1) as isize
    }

    fn selected_link(&self) -> Option<LinkRef> {
        self.document()
            .get_links()
            .get(self.link_selection_index as usize)
            .cloned()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum NotePaneState {
    #[default]
    NoNote,
    WithNote(NoteContext),
    LoadingNote(NoteKey),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NotePaneModel {
    state: NotePaneState,
    history: Vec<NoteKey>,
}

impl NotePaneState {
    fn from_note(note: NoteSnapshot) -> Self {
        Self::WithNote(NoteContext {
            note,
            scroll_lines: 0,
            link_selection_index: 0,
        })
    }
    fn context(&self) -> Option<NoteContext> {
        if let Self::WithNote(context) = self {
            Some(context.clone())
        } else {
            None
        }
    }
    fn key(&self) -> Option<&NoteKey> {
        match self {
            Self::LoadingNote(note) => Some(note),
            Self::WithNote(context) => Some(&context.note.key),
            Self::NoNote => None,
        }
    }
}

impl Update<NotePaneMessage> for NotePaneModel {
    fn update(&self, message: NotePaneMessage) -> (Self, Command) {
        let mut model = self.clone();
        let mut command = Command::None;

        if let NotePaneMessage::PushNote(note_snapshot) = &message {
            if let Some(key) = self.state.key() {
                model.history.push(key.clone());
            }
            model.state = NotePaneState::from_note(note_snapshot.clone());
        }
        if let NotePaneMessage::UpdateNote(note_snapshot) = &message {
            model.state = NotePaneState::from_note(note_snapshot.clone());
        }
        if let NotePaneMessage::PopNote = &message {
            let new_note = model.history.pop();

            model.state = new_note
                .clone()
                .map(NotePaneState::LoadingNote)
                .unwrap_or(NotePaneState::NoNote);

            command = new_note.map(Command::ServeNote).unwrap_or(Command::None);
        }

        if let NotePaneState::WithNote(context) = &mut model.state {
            let delta_scroll = match message {
                NotePaneMessage::ScrollUp => -1,
                NotePaneMessage::ScrollDown => 1,
                _ => 0,
            };

            let delta_link_selection = match message {
                NotePaneMessage::NextLink => 1,
                NotePaneMessage::PreviousLink => -1,
                _ => 0,
            };

            context.scroll_lines =
                context
                    .scroll_lines
                    .add_clamped(delta_scroll, 0, context.max_scroll());

            context.link_selection_index = context.link_selection_index.add_clamped(
                delta_link_selection,
                0,
                context.max_link_selection(),
            );

            if let NotePaneMessage::FollowLink = message {
                command = context
                    .selected_link()
                    .map(|link_ref| Command::FollowLink(link_ref.target))
                    .unwrap_or(Command::None);
            }

            if let NotePaneMessage::EditExternally = message {
                command = Command::EditExternally(context.note.key.to_owned());
            }
        }

        (model, command)
    }
}

pub enum NotePaneMessage {
    PushNote(NoteSnapshot),
    PopNote,
    UpdateNote(NoteSnapshot),
    ScrollUp,
    ScrollDown,
    NextLink,
    PreviousLink,
    FollowLink,
    EditExternally,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_model() {
        assert_eq!(
            Model::default(),
            Model {
                running_state: RunningState::Running,
                note_pane: NotePaneModel::default()
            }
        )
    }

    #[test]
    fn test_quit() {
        let model = Model::default();

        assert_eq!(model.running_state, RunningState::Running);
        let (model, _) = model.update(Message::Quit);
        assert_eq!(model.running_state, RunningState::Done);
    }

    mod note_view_model {
        use crate::repository::MockRepository;

        use super::*;

        #[test]
        fn test_default_model() {
            assert_eq!(
                NotePaneModel::default(),
                NotePaneModel {
                    state: NotePaneState::NoNote,
                    history: vec![]
                }
            );
        }

        #[test]
        fn test_open_note() {
            let model = NotePaneModel::default();

            let note = MockRepository::new().insert_note("note", "This is a note");

            assert_eq!(model.state, NotePaneState::NoNote);
            let (model, _) = model.update(NotePaneMessage::PushNote(note.clone()));
            assert_eq!(
                model.state,
                NotePaneState::WithNote(NoteContext {
                    note: note,
                    scroll_lines: 0,
                    link_selection_index: 0,
                })
            );

            let new_note = MockRepository::new().insert_note("note2", "This is another note");

            let (model, _) = model.update(NotePaneMessage::PushNote(new_note.clone()));
            assert_eq!(
                model.state,
                NotePaneState::WithNote(NoteContext {
                    note: new_note,
                    scroll_lines: 0,
                    link_selection_index: 0
                })
            );
        }

        #[test]
        fn test_scroll() {
            let model = NotePaneModel::default();

            let note = MockRepository::new().insert_note("note", "This has\nmultiple\nlines");
            let (model, _) = model.update(NotePaneMessage::PushNote(note));

            assert_eq!(
                model.state.context().map(|context| context.scroll_lines),
                Some(0)
            );
            let (model, _) = model.update(NotePaneMessage::ScrollDown);
            assert_eq!(
                model.state.context().map(|context| context.scroll_lines),
                Some(1)
            );
            let (model, _) = model.update(NotePaneMessage::ScrollDown);
            assert_eq!(
                model.state.context().map(|context| context.scroll_lines),
                Some(2)
            );
            let (model, _) = model.update(NotePaneMessage::ScrollDown);
            // Max scroll down reached.
            assert_eq!(
                model.state.context().map(|context| context.scroll_lines),
                Some(2)
            );
            let (model, _) = model.update(NotePaneMessage::ScrollUp);
            assert_eq!(
                model.state.context().map(|context| context.scroll_lines),
                Some(1)
            );
            let (model, _) = model.update(NotePaneMessage::ScrollUp);
            assert_eq!(
                model.state.context().map(|context| context.scroll_lines),
                Some(0)
            );
            let (model, _) = model.update(NotePaneMessage::ScrollUp);
            // Max scroll up reached.
            assert_eq!(
                model.state.context().map(|context| context.scroll_lines),
                Some(0)
            );
        }

        #[test]
        fn test_link_selection() {
            let model = NotePaneModel::default();

            let note = MockRepository::new().insert_note(
                "Note name.md",
                "[[This]] file [[Have|has]] multiple [links](url.com)",
            );
            let (model, _) = model.update(NotePaneMessage::PushNote(note));

            assert_eq!(
                model
                    .state
                    .context()
                    .map(|context| context.link_selection_index),
                Some(0)
            );
            let (model, _) = model.update(NotePaneMessage::NextLink);
            assert_eq!(
                model
                    .state
                    .context()
                    .map(|context| context.link_selection_index),
                Some(1)
            );
            let (model, _) = model.update(NotePaneMessage::NextLink);
            assert_eq!(
                model
                    .state
                    .context()
                    .map(|context| context.link_selection_index),
                Some(2)
            );
            let (model, _) = model.update(NotePaneMessage::NextLink);
            assert_eq!(
                model
                    .state
                    .context()
                    .map(|context| context.link_selection_index),
                Some(2)
            );
            let (model, _) = model.update(NotePaneMessage::PreviousLink);
            assert_eq!(
                model
                    .state
                    .context()
                    .map(|context| context.link_selection_index),
                Some(1)
            );
            let (model, _) = model.update(NotePaneMessage::PreviousLink);
            assert_eq!(
                model
                    .state
                    .context()
                    .map(|context| context.link_selection_index),
                Some(0)
            );
            let (model, _) = model.update(NotePaneMessage::PreviousLink);
            assert_eq!(
                model
                    .state
                    .context()
                    .map(|context| context.link_selection_index),
                Some(0)
            );
        }

        #[test]
        fn test_follow_link() {
            let model = NotePaneModel::default();

            let note = MockRepository::new()
                .insert_note("note A.md", "[[note A|This]] has a link to [[note B]].");

            let (model, _) = model.update(NotePaneMessage::PushNote(note));
            let (model, _) = model.update(NotePaneMessage::NextLink);

            let (_, command) = model.update(NotePaneMessage::FollowLink);
            assert_eq!(
                command,
                Command::FollowLink(LinkTarget::Note(String::from("note B")))
            );
        }

        #[test]
        fn test_edit_externally() {
            let model = NotePaneModel::default();

            let note = MockRepository::new().insert_note("note.md", "This is a note.");
            let note_key = note.key.clone();

            let (model, _) = model.update(NotePaneMessage::PushNote(note));

            let (model, command) = model.update(NotePaneMessage::EditExternally);
            assert_eq!(command, Command::EditExternally(note_key));

            let note = MockRepository::new().insert_note("note.md", "This not has been updated.");
            let (model, _) = model.update(NotePaneMessage::UpdateNote(note.clone()));
            assert_eq!(
                model.state.context().map(|context| context.note),
                Some(note)
            );
        }

        #[test]
        fn test_history() {
            let model = NotePaneModel::default();

            let note_a = MockRepository::new().insert_note("note A.md", "This is a note.");
            let note_b = MockRepository::new().insert_note("note B.md", "This is a note.");
            let note_c = MockRepository::new().insert_note("note C.md", "This is a note.");

            let (model, _) = model.update(NotePaneMessage::PushNote(note_a.clone()));
            let (model, _) = model.update(NotePaneMessage::PushNote(note_b.clone()));
            let (model, _) = model.update(NotePaneMessage::PushNote(note_c.clone()));

            let (model, command) = model.update(NotePaneMessage::PopNote);
            assert_eq!(model.state, NotePaneState::LoadingNote(note_b.key.clone()));
            assert_eq!(command, Command::ServeNote(note_b.key.clone()));

            let (model, _) = model.update(NotePaneMessage::UpdateNote(note_b));
            let (model, command) = model.update(NotePaneMessage::PopNote);
            assert_eq!(model.state, NotePaneState::LoadingNote(note_a.key.clone()));
            assert_eq!(command, Command::ServeNote(note_a.key.clone()));

            let (model, _) = model.update(NotePaneMessage::UpdateNote(note_a));
            let (model, command) = model.update(NotePaneMessage::PopNote);
            assert_eq!(model.state, NotePaneState::NoNote);
            assert_eq!(command, Command::None);
        }
    }
}
