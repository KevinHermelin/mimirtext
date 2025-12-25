use crate::{
    graph::SearchResult,
    model::{ClampAdd, Command, Update},
    text_input::{InputOperation, TextInput},
};

#[derive(Clone, Debug, PartialEq)]
pub struct SearchWindowModel {
    pub input: TextInput,
    pub results: Vec<SearchResult>,
    pub selection_index: isize,
    pub repo_id: String,
}

impl SearchWindowModel {
    pub fn new(repo_id: &str) -> Self {
        Self {
            input: TextInput::new(),
            results: vec![],
            selection_index: 0,
            repo_id: repo_id.to_owned(),
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

#[cfg(test)]
mod tests {
    use crate::{
        repository::{MockRepository, Repository},
        text_input::Unit,
    };

    use super::*;

    #[test]
    fn test_default_model() {
        assert_eq!(
            SearchWindowModel::new("repo"),
            SearchWindowModel {
                input: TextInput::new(),
                results: vec![],
                selection_index: 0,
                repo_id: String::from("repo")
            }
        );
    }

    #[test]
    fn test_input() {
        let model = SearchWindowModel {
            selection_index: 2,
            ..SearchWindowModel::new("repo")
        };

        let (model, command) = model.update(SearchWindowMessage::Input(InputOperation::Insert(
            String::from("Hello Word?"),
        )));
        assert_eq!(model.input.text(), "Hello Word?");
        assert_eq!(command, Command::SearchQuery(String::from("Hello Word?")));
        // Selection index should be reset.
        assert_eq!(model.selection_index, 0);

        let (model, _) = model.update(SearchWindowMessage::Input(InputOperation::Backspace));
        let (model, _) = model.update(SearchWindowMessage::Input(InputOperation::Left(Unit::Char)));
        let (model, command) = model.update(SearchWindowMessage::Input(InputOperation::Insert(
            String::from("l"),
        )));
        assert_eq!(model.input.text(), "Hello World");
        assert_eq!(command, Command::SearchQuery(String::from("Hello World")));
    }

    #[test]
    fn test_update_results() {
        let repo = MockRepository::new();
        let model = SearchWindowModel::new(repo.id());

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
        let repo = MockRepository::new();
        let model = SearchWindowModel::new(repo.id());

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
