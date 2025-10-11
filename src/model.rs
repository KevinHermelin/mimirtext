pub mod note_pane;
pub mod search_window;

use std::ops::Range;

use crate::{
    markdown::LinkTarget,
    model::{
        note_pane::{NotePaneMessage, NotePaneModel},
        search_window::{SearchWindowMessage, SearchWindowModel},
    },
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

#[cfg(test)]
mod tests {
    use crate::repository::{MockRepository, Repository, SearchResult};

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
}
