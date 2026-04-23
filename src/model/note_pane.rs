use crate::{
    document::{DocumentType, LinkTarget},
    graph::builder::GraphBuildProgress,
    model::{ClampAdd, Command, Update},
    repository::{NoteKey, NoteSnapshot},
    text_input::{Completion, InputOperation, TextInput, TextInputConfig},
};

/// A struct representing the state of a note opened for reading and optionally writing.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteContext {
    pub note: NoteSnapshot,
    pub scroll_lines: isize,
    pub link_selection_index: isize,
    /// Optionally, a `TextInput` instance holding the edited state of this note.
    pub editor: Option<TextInput>,
}

impl NoteContext {
    /// Creates a `NoteContext` containing a note snapshot.
    ///
    /// The context is initialized without an editor.
    pub fn new(note: NoteSnapshot) -> Self {
        Self {
            note,
            scroll_lines: 0,
            link_selection_index: 0,
            editor: None,
        }
    }

    /// Returns `true` if the editor is active and its text does not match the note snapshot.
    pub fn modified(&self) -> bool {
        self.editor
            .as_ref()
            .is_some_and(|editor| editor.text() != self.note.body)
    }

    fn max_scroll(&self) -> isize {
        self.note.body.lines().count().saturating_sub(1) as isize
    }

    fn max_link_selection(&self) -> isize {
        self.note
            .parse()
            .as_document()
            .links()
            .len()
            .saturating_sub(1) as isize
    }

    pub fn selected_link(&self) -> Option<LinkTarget> {
        self.note
            .parse()
            .as_document()
            .links()
            .get(self.link_selection_index as usize)
            .cloned()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum NotePaneState {
    #[default]
    Empty,
    With(NoteContext),
    Loading(NoteKey),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NotePaneModel {
    pub state: NotePaneState,
    pub history: Vec<NoteKey>,
    pub graph_progress: GraphBuildProgress,
}

#[derive(Debug, PartialEq)]
pub enum ViewMode {
    None,
    Source,
    Browsing,
    Edit,
}

impl NotePaneModel {
    pub fn editor_config(&self) -> TextInputConfig {
        TextInputConfig::default()
    }
    pub fn view_mode(&self) -> ViewMode {
        if let Some(context) = self.state.context() {
            if context.editor.is_some() {
                return ViewMode::Edit;
            }

            return match context.note.parse() {
                DocumentType::Markdown(_) => ViewMode::Browsing,
                DocumentType::Source(_) => ViewMode::Source,
            };
        }
        ViewMode::None
    }
}

impl NotePaneState {
    fn from_note(note: NoteSnapshot) -> Self {
        Self::With(NoteContext::new(note))
    }
    fn context(&self) -> Option<NoteContext> {
        if let Self::With(context) = self {
            Some(context.clone())
        } else {
            None
        }
    }
    fn key(&self) -> Option<&NoteKey> {
        match self {
            Self::Loading(note) => Some(note),
            Self::With(context) => Some(&context.note.key),
            Self::Empty => None,
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
                .map(NotePaneState::Loading)
                .unwrap_or(NotePaneState::Empty);

            command = new_note.map(Command::ServeNote).unwrap_or(Command::None);
        }

        if let NotePaneMessage::GraphUpdate(graph_progress) = &message {
            model.graph_progress = graph_progress.clone();
        }

        if let NotePaneState::With(context) = &mut model.state {
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

            if let NotePaneMessage::StartEdit = message {
                context.editor = Some(
                    TextInput::from(context.note.body.as_str()).with_config(self.editor_config()),
                );
            }

            if let NotePaneMessage::StopEdit = message {
                if let Some(editor) = &context.editor {
                    context.note.body = editor.text();
                    command = Command::CommitNote(context.note.clone())
                }
                context.editor = None;
            }

            if let NotePaneMessage::Input(operation) = &message {
                if let Some(editor) = &context.editor {
                    let editor = editor.clone().apply(operation.clone());

                    // Search for unfinished wiki links, i.e. "[[" without a closing "]]", on the current line.
                    let search_text = editor.before_cursor().split('\n').next_back().unwrap();
                    let search_text = search_text.split("]]").last().unwrap();
                    // If there is any "[[" in search text now, it means that they have not been closed
                    // up until the cursor. This means that we should trigger autocomplete.
                    if search_text.contains("[[") {
                        let search_text = search_text.split("[[").last().unwrap();
                        let link_start_index = editor.before_cursor().rfind("[[").unwrap();
                        command = Command::RequestLinkCompletion(
                            link_start_index..editor.cursor_pos(),
                            search_text.to_owned(),
                        )
                    }

                    context.editor = Some(editor);
                }
            }

            if let NotePaneMessage::UpdateCompletion(completions) = &message {
                let editor = context
                    .editor
                    .clone()
                    .expect("should have an editor if provided with completions");
                context.editor = Some(editor.provide_completions(completions.clone()));
            }

            if let NotePaneMessage::FollowLink = message {
                command = context
                    .selected_link()
                    .map(Command::FollowLink)
                    .unwrap_or(Command::None);
            }

            if let NotePaneMessage::EditExternally = message {
                command = Command::EditExternally(context.note.key.to_owned());
            }
        }

        (model, command)
    }
}

#[derive(Clone)]
pub enum NotePaneMessage {
    PushNote(NoteSnapshot),
    PopNote,
    UpdateNote(NoteSnapshot),
    GraphUpdate(GraphBuildProgress),
    ScrollUp,
    ScrollDown,
    NextLink,
    PreviousLink,
    FollowLink,
    EditExternally,
    StartEdit,
    StopEdit,
    Input(InputOperation),
    UpdateCompletion(Vec<Completion>),
    None,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{document::LinkTarget, repository::mock::MockRepository};

    #[test]
    fn test_default_model() {
        assert_eq!(
            NotePaneModel::default(),
            NotePaneModel {
                state: NotePaneState::Empty,
                history: vec![],
                graph_progress: GraphBuildProgress::Idle
            }
        );
    }

    #[test]
    fn test_new_context() {
        let note = MockRepository::new().insert_note("note", "This is a note");

        assert_eq!(
            NoteContext::new(note.clone()),
            NoteContext {
                note,
                scroll_lines: 0,
                link_selection_index: 0,
                editor: None,
            }
        );
    }

    #[test]
    fn test_open_note() {
        let model = NotePaneModel::default();

        let note = MockRepository::new().insert_note("note", "This is a note");

        assert_eq!(model.state, NotePaneState::Empty);
        let (model, _) = model.update(NotePaneMessage::PushNote(note.clone()));
        assert_eq!(model.state, NotePaneState::With(NoteContext::new(note)));

        let new_note = MockRepository::new().insert_note("note2", "This is another note");

        let (model, _) = model.update(NotePaneMessage::PushNote(new_note.clone()));
        assert_eq!(model.state, NotePaneState::With(NoteContext::new(new_note)));
    }

    #[test]
    fn test_source_mode() {
        let note = MockRepository::new().insert_note("note without extension", "This is a note");
        let (model, _) = NotePaneModel::default().update(NotePaneMessage::PushNote(note));

        // The note pane should go into source mode because the note has no supported extension.
        assert_eq!(model.view_mode(), ViewMode::Source);
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
    fn test_edit_mode() {
        let note = MockRepository::new().insert_note("note.md", "This is a note.");

        let (model, _) = NotePaneModel::default().update(NotePaneMessage::PushNote(note.clone()));
        assert_eq!(model.view_mode(), ViewMode::Browsing);
        assert_eq!(model.state.context().unwrap().editor, None);
        assert!(!model.state.context().unwrap().modified());

        let (model, _) = model.update(NotePaneMessage::StartEdit);
        assert_eq!(model.view_mode(), ViewMode::Edit);
        assert_eq!(
            model.state.context().unwrap().editor,
            Some(TextInput::new_with("This is a note.", 0))
        );
        assert!(!model.state.context().unwrap().modified());

        let (model, _) = model.update(NotePaneMessage::Input(InputOperation::Insert(
            String::from("This is an edit.\n"),
        )));
        assert_eq!(
            model.state.context().unwrap().editor,
            Some(TextInput::new_with(
                "This is an edit.\nThis is a note.",
                "This is an edit.\n".len()
            ))
        );
        assert!(model.state.context().unwrap().modified());

        let (model, command) = model.update(NotePaneMessage::StopEdit);
        assert_eq!(model.view_mode(), ViewMode::Browsing);
        assert_eq!(model.state.context().unwrap().editor, None);

        let mut new_note = note;
        new_note.body = String::from("This is an edit.\nThis is a note.");
        assert_eq!(command, Command::CommitNote(new_note))
    }

    #[test]
    fn test_link_completions() {
        let note = MockRepository::new()
            .insert_note("note.md", "\nNote where we want to add a [[Link|link]].");

        let model = NotePaneModel::default();
        let (model, _) = model.update(NotePaneMessage::PushNote(note));
        let (model, _) = model.update(NotePaneMessage::StartEdit);
        let (model, command) = model.update(NotePaneMessage::Input(InputOperation::Insert(
            String::from("Add a [["),
        )));

        assert_eq!(
            command,
            Command::RequestLinkCompletion("Add a ".len().."Add a [[".len(), String::from(""))
        );

        let completions = vec![Completion::note_link(
            "Add a ".len().."Add a [[".len(),
            "note",
        )];

        let (model, _) = model.update(NotePaneMessage::UpdateCompletion(completions.clone()));
        assert_eq!(
            *model
                .state
                .context()
                .unwrap()
                .editor
                .unwrap()
                .completions()
                .as_ref()
                .unwrap()
                .items(),
            completions
        );

        let (_, command) = model.update(NotePaneMessage::Input(InputOperation::Insert(
            String::from("no"),
        )));
        assert_eq!(
            command,
            Command::RequestLinkCompletion("Add a ".len().."Add a [[no".len(), String::from("no"))
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
        assert_eq!(model.state, NotePaneState::Loading(note_b.key.clone()));
        assert_eq!(command, Command::ServeNote(note_b.key.clone()));

        let (model, _) = model.update(NotePaneMessage::UpdateNote(note_b));
        let (model, command) = model.update(NotePaneMessage::PopNote);
        assert_eq!(model.state, NotePaneState::Loading(note_a.key.clone()));
        assert_eq!(command, Command::ServeNote(note_a.key.clone()));

        let (model, _) = model.update(NotePaneMessage::UpdateNote(note_a));
        let (model, command) = model.update(NotePaneMessage::PopNote);
        assert_eq!(model.state, NotePaneState::Empty);
        assert_eq!(command, Command::None);
    }

    #[test]
    fn test_graph_update() {
        let model = NotePaneModel::default();
        assert_eq!(model.graph_progress, GraphBuildProgress::Idle);

        let (model, _) = model.update(NotePaneMessage::GraphUpdate(
            GraphBuildProgress::InProgress(0.1),
        ));
        assert_eq!(model.graph_progress, GraphBuildProgress::InProgress(0.1));

        let (model, _) = model.update(NotePaneMessage::GraphUpdate(GraphBuildProgress::Idle));
        assert_eq!(model.graph_progress, GraphBuildProgress::Idle);
    }
}
