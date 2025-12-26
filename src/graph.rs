use std::collections::HashSet;

use rapidfuzz::distance::jaro_winkler;

use crate::{
    document::{Document, LinkTarget, markdown::MarkdownDocument},
    repository::{NoteKey, NoteSnapshot},
};

pub struct RepositoryGraph {
    labels: HashSet<NoteKey>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub key: NoteKey,
    pub distance: f64,
}

impl SearchResult {
    pub fn new(key: NoteKey, distance: f64) -> Self {
        SearchResult { key, distance }
    }
}

impl RepositoryGraph {
    pub fn new() -> Self {
        Self {
            labels: HashSet::new(),
        }
    }

    pub fn register_note(mut self, note: &NoteSnapshot) -> Self {
        let NoteSnapshot { key, body, .. } = note.to_owned();

        let NoteKey(repo_id, _) = key.clone();

        // Register this note.
        self.labels.insert(key);

        // Register outgoing links.
        let outgoing = MarkdownDocument::new(&body).links();
        for link in outgoing {
            if let LinkTarget::Note(title) = link {
                let key = NoteKey(repo_id.clone(), title + ".md");
                self.labels.insert(key);
            }
        }

        self
    }

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let scorer = jaro_winkler::BatchComparator::new(query.to_lowercase().chars());

        let mut labels: Vec<SearchResult> = self
            .labels
            .iter()
            .map(|NoteKey(repo_id, note_id)| {
                SearchResult::new(
                    NoteKey(repo_id.to_string(), note_id.to_string()),
                    scorer.distance(note_id.to_lowercase().chars()),
                )
            })
            .collect();

        if query.is_empty() {
            labels.sort_by_key(|result| result.key.1.clone());
        } else {
            labels.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
        }

        labels
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::mock::MockRepository;

    #[test]
    fn test_label_search() {
        let mut repo = MockRepository::new();
        let graph = RepositoryGraph::new();

        assert_eq!(graph.search("note"), []);

        let note = repo.insert_note("note.md", "This points to [[another note]]. Two links to make sure that it removes duplicates [[another note]].");
        let graph = graph.register_note(&note);

        let note_key = note.key;
        let another_note_key = NoteKey(note_key.clone().0, String::from("another note.md"));

        let search_results: Vec<NoteKey> = graph
            .search("note")
            .iter()
            .map(|result| result.key.clone())
            .collect();
        assert_eq!(search_results, vec![note_key, another_note_key]);
    }

    #[test]
    fn test_label_search_empty_query() {
        let mut repo = MockRepository::new();
        let graph = RepositoryGraph::new();

        assert_eq!(graph.search(""), []);

        let note = repo.insert_note("note.md", "This points to [[another note]]. Two links to make sure that it removes duplicates [[another note]].");
        let graph = graph.register_note(&note);

        let note_key: NoteKey = note.key;
        let another_note_key = NoteKey(note_key.clone().0, String::from("another note.md"));

        let search_results: Vec<NoteKey> = graph
            .search("")
            .iter()
            .map(|result| result.key.clone())
            .collect();

        // Results should be in alphabetical order.
        assert_eq!(search_results, vec![another_note_key, note_key]);
    }
}
