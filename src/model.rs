use std::ops::Range;

use crate::{
    markdown::{LinkRef, LinkTarget, MarkdownDocument},
    repository::{NoteKey, NoteSnapshot, SearchResult},
    text_input::{Completion, InputOperation, TextInput, TextInputConfig},
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
    pub search_window: Option<SearchWindowModel>,
}

#[derive(Clone)]
pub enum Message {
    Quit,
    NotePane(NotePaneMessage),
    None,
    SearchWindow(SearchWindowMessage),
    OpenSearch,
    CloseSearch,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Command {
    #[default]
    None,
    FollowLink(LinkTarget),
    ServeNote(NoteKey),
    OpenNote(NoteKey),
    EditExternally(NoteKey),
    SearchQuery(String),
    CommitNote(NoteSnapshot),
    RequestLinkCompletion(Range<usize>, String),
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
            Message::SearchWindow(message) => {
                if let Some(mut search_window) = model.search_window {
                    (search_window, command) = search_window.update(message);
                    if let Command::OpenNote(_) = command {
                        // This means that a search result has been selected and that
                        // we should close the search window. This is not a good approach,
                        // mostly because "OpenNote" does not tell us why the note should be
                        // opened. There is also the question of whether we should intercept
                        // commands like this at all.
                        model.search_window = None;
                    } else {
                        model.search_window = Some(search_window)
                    }
                }
            }
            Message::OpenSearch => model.search_window = Some(SearchWindowModel::default()),
            Message::CloseSearch => model.search_window = None,
            Message::None => {}
        }
        (model, command)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchWindowModel {
    pub input: TextInput,
    pub results: Vec<SearchResult>,
    pub selection_index: isize,
}

impl Default for SearchWindowModel {
    fn default() -> Self {
        Self {
            input: TextInput::new(),
            results: vec![],
            selection_index: 0,
        }
    }
}

#[derive(Clone)]
pub enum SearchWindowMessage {
    Input(InputOperation),
    UpdateResults(Vec<SearchResult>),
    NextResult,
    PreviousResult,
    OpenResult,
}

impl Update<SearchWindowMessage> for SearchWindowModel {
    fn update(&self, message: SearchWindowMessage) -> (Self, Command) {
        let mut model = self.clone();
        let mut command = Command::None;

        match &message {
            SearchWindowMessage::Input(operation) => {
                model.input = model.input.apply(operation.to_owned());
                command = Command::SearchQuery(model.input.text())
            }
            SearchWindowMessage::UpdateResults(results) => model.results = results.to_owned(),
            SearchWindowMessage::OpenResult => {
                command = model
                    .results
                    .get(model.selection_index as usize)
                    .cloned()
                    .map(|result| result.key)
                    .map(Command::OpenNote)
                    .unwrap_or_default()
            }
            _ => {}
        }

        let delta_selection = match &message {
            SearchWindowMessage::PreviousResult => -1,
            SearchWindowMessage::NextResult => 1,
            _ => 0,
        };

        model.selection_index = (model.selection_index).add_clamped(
            delta_selection,
            0,
            model.results.len().saturating_sub(1) as isize,
        );

        (model, command)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NoteContext {
    pub note: NoteSnapshot,
    pub scroll_lines: isize,
    pub link_selection_index: isize,
    pub editor: Option<TextInput>,
}

#[derive(Debug, PartialEq)]
pub enum Document {
    Markdown(MarkdownDocument),
    Source(String),
}

impl NoteContext {
    pub fn new(note: NoteSnapshot) -> Self {
        Self {
            note,
            scroll_lines: 0,
            link_selection_index: 0,
            editor: None,
        }
    }

    pub fn document(&self) -> Document {
        if self.note.extension == Some(String::from("md")) {
            Document::Markdown(MarkdownDocument::new(&self.note.body))
        } else {
            Document::Source(self.note.body.clone())
        }
    }

    fn max_scroll(&self) -> isize {
        self.note.body.lines().count().saturating_sub(1) as isize
    }

    fn max_link_selection(&self) -> isize {
        match self.document() {
            Document::Markdown(markdown_document) => markdown_document
                .get_links()
                .iter()
                .count()
                .saturating_sub(1) as isize,
            Document::Source(_) => 0,
        }
    }

    pub fn selected_link(&self) -> Option<LinkRef> {
        match self.document() {
            Document::Markdown(markdown_document) => markdown_document
                .get_links()
                .get(self.link_selection_index as usize)
                .cloned(),
            Document::Source(_) => None,
        }
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
    pub state: NotePaneState,
    pub history: Vec<NoteKey>,
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
            if let Some(_) = context.editor {
                return ViewMode::Edit;
            }

            return match context.document() {
                Document::Markdown(_) => ViewMode::Browsing,
                Document::Source(_) => ViewMode::Source,
            };
        }
        ViewMode::None
    }
}

impl NotePaneState {
    fn from_note(note: NoteSnapshot) -> Self {
        Self::WithNote(NoteContext::new(note))
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
                    let search_text = editor.before_cursor().split('\n').last().unwrap();
                    let search_text = search_text.split("]]").last().unwrap();
                    // If there is any "[[" in search text now, it means that they have not been closed
                    // up until the cursor. This means that we should trigger autocomplete.
                    if search_text.contains("[[") {
                        let search_text = search_text.split("[[").last().unwrap();
                        let index = editor.text().rfind("[[").unwrap();
                        command = Command::RequestLinkCompletion(
                            index..editor.cursor_pos(),
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

#[derive(Clone)]
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
    StartEdit,
    StopEdit,
    Input(InputOperation),
    UpdateCompletion(Vec<Completion>),
    None,
}

#[cfg(test)]
mod tests {
    use crate::repository::{MockRepository, Repository};

    use super::*;

    #[test]
    fn test_default_model() {
        assert_eq!(
            Model::default(),
            Model {
                running_state: RunningState::Running,
                note_pane: NotePaneModel::default(),
                search_window: None
            }
        )
    }

    #[test]
    fn test_search_window() {
        let model = Model::default();
        assert_eq!(model.search_window, None);
        let (model, _) = model.update(Message::OpenSearch);
        assert_eq!(model.search_window, Some(SearchWindowModel::default()));
        let (model, _) = model.update(Message::CloseSearch);
        assert_eq!(model.search_window, None);

        let (model, _) = model.update(Message::OpenSearch);
        assert_eq!(model.search_window, Some(SearchWindowModel::default()));

        let (model, _) = model.update(Message::SearchWindow(SearchWindowMessage::UpdateResults(
            vec![SearchResult::new(
                MockRepository::new().note_key("search_result"),
                1.0,
            )],
        )));
        let (model, _) = model.update(Message::SearchWindow(SearchWindowMessage::OpenResult));
        assert_eq!(model.search_window, None);
    }

    #[test]
    fn test_quit() {
        let model = Model::default();

        assert_eq!(model.running_state, RunningState::Running);
        let (model, _) = model.update(Message::Quit);
        assert_eq!(model.running_state, RunningState::Done);
    }

    mod search_window {
        use crate::repository::{MockRepository, Repository};

        use super::*;

        #[test]
        fn test_default_model() {
            assert_eq!(
                SearchWindowModel::default(),
                SearchWindowModel {
                    input: TextInput::new(),
                    results: vec![],
                    selection_index: 0
                }
            );
        }

        #[test]
        fn test_input() {
            let mut model = SearchWindowModel::default();
            model.selection_index = 2;

            let (model, command) = model.update(SearchWindowMessage::Input(
                InputOperation::Insert(String::from("Hello Word?")),
            ));
            assert_eq!(model.input.text(), "Hello Word?");
            assert_eq!(command, Command::SearchQuery(String::from("Hello Word?")));
            // Selection index should be reset.
            assert_eq!(model.selection_index, 0);

            let (model, _) = model.update(SearchWindowMessage::Input(InputOperation::Backspace));
            let (model, _) = model.update(SearchWindowMessage::Input(InputOperation::Left));
            let (model, command) = model.update(SearchWindowMessage::Input(
                InputOperation::Insert(String::from("l")),
            ));
            assert_eq!(model.input.text(), "Hello World");
            assert_eq!(command, Command::SearchQuery(String::from("Hello World")));
        }

        #[test]
        fn test_update_results() {
            let model = SearchWindowModel::default();
            let repo = MockRepository::new();

            let (model, _) = model.update(SearchWindowMessage::UpdateResults(vec![
                SearchResult::new(repo.note_key("search_result_a"), 1.0),
                SearchResult::new(repo.note_key("search_result_b"), 1.0),
                SearchResult::new(repo.note_key("search_result_c"), 1.0),
            ]));

            assert_eq!(
                model.results,
                vec![
                    SearchResult::new(repo.note_key("search_result_a"), 1.0),
                    SearchResult::new(repo.note_key("search_result_b"), 1.0),
                    SearchResult::new(repo.note_key("search_result_c"), 1.0),
                ]
            )
        }

        #[test]
        fn test_result_selection() {
            let model = SearchWindowModel::default();
            let repo = MockRepository::new();

            let (model, _) = model.update(SearchWindowMessage::UpdateResults(vec![
                SearchResult::new(repo.note_key("search_result_a"), 1.0),
                SearchResult::new(repo.note_key("search_result_b"), 1.0),
                SearchResult::new(repo.note_key("search_result_c"), 1.0),
            ]));
            assert_eq!(model.selection_index, 0);
            let (model, _) = model.update(SearchWindowMessage::PreviousResult);
            assert_eq!(model.selection_index, 0);

            let (model, _) = model.update(SearchWindowMessage::NextResult);
            let (model, _) = model.update(SearchWindowMessage::NextResult);
            assert_eq!(model.selection_index, 2);
            let (model, _) = model.update(SearchWindowMessage::NextResult);
            assert_eq!(model.selection_index, 2);

            let (model, _) = model.update(SearchWindowMessage::PreviousResult);
            assert_eq!(model.selection_index, 1);

            let (_, command) = model.update(SearchWindowMessage::OpenResult);
            assert_eq!(command, Command::OpenNote(repo.note_key("search_result_b")));
        }
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

            assert_eq!(model.state, NotePaneState::NoNote);
            let (model, _) = model.update(NotePaneMessage::PushNote(note.clone()));
            assert_eq!(model.state, NotePaneState::WithNote(NoteContext::new(note)));

            let new_note = MockRepository::new().insert_note("note2", "This is another note");

            let (model, _) = model.update(NotePaneMessage::PushNote(new_note.clone()));
            assert_eq!(
                model.state,
                NotePaneState::WithNote(NoteContext::new(new_note))
            );
        }

        #[test]
        fn test_source_mode() {
            let note =
                MockRepository::new().insert_note("note without extension", "This is a note");
            let (model, _) = NotePaneModel::default().update(NotePaneMessage::PushNote(note));

            // The note pane should go into source mode because the note has no supported extension.
            assert_eq!(model.view_mode(), ViewMode::Source);
        }

        #[test]
        fn test_document() {
            let context =
                NoteContext::new(MockRepository::new().insert_note("note.md", "This is a note"));
            assert_eq!(
                context.document(),
                Document::Markdown(MarkdownDocument::new("This is a note"))
            );

            let context = NoteContext::new(
                MockRepository::new().insert_note("note without extension", "This is also a note"),
            );
            assert_eq!(
                context.document(),
                Document::Source(String::from("This is also a note"))
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
        fn test_edit_mode() {
            let note = MockRepository::new().insert_note("note.md", "This is a note.");

            let (model, _) =
                NotePaneModel::default().update(NotePaneMessage::PushNote(note.clone()));
            assert_eq!(model.view_mode(), ViewMode::Browsing);
            assert_eq!(model.state.context().unwrap().editor, None);

            let (model, _) = model.update(NotePaneMessage::StartEdit);
            assert_eq!(model.view_mode(), ViewMode::Edit);
            assert_eq!(
                model.state.context().unwrap().editor,
                Some(TextInput::new_with("This is a note.", 0))
            );

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

            let (model, command) = model.update(NotePaneMessage::StopEdit);
            assert_eq!(model.view_mode(), ViewMode::Browsing);
            assert_eq!(model.state.context().unwrap().editor, None);

            let mut new_note = note;
            new_note.body = String::from("This is an edit.\nThis is a note.");
            assert_eq!(command, Command::CommitNote(new_note))
        }

        #[test]
        fn test_link_completions() {
            let note = MockRepository::new().insert_note("note.md", "");

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
                Command::RequestLinkCompletion(
                    "Add a ".len().."Add a [[no".len(),
                    String::from("no")
                )
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
