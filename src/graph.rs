use rapidfuzz::distance::jaro_winkler;
use rusqlite::Connection;

use crate::{
    markdown::{LinkTarget, MarkdownDocument},
    repository::{NoteKey, NoteSnapshot},
};

pub struct RepositoryGraph {
    connection: Connection,
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
    pub fn new() -> rusqlite::Result<Self> {
        let connection = Connection::open_in_memory()?;

        connection.execute(
            "CREATE TABLE label (
                    id      INTEGER PRIMARY KEY,
                    title   TEXT NOT NULL,
                    repo    TEXT NOT NULL
                )",
            (),
        )?;

        Ok(Self { connection })
    }

    fn get_labels(&self) -> rusqlite::Result<Vec<NoteKey>> {
        let mut stmt: rusqlite::Statement<'_> = self
            .connection
            .prepare("SELECT id, title, repo FROM label")?;

        let labels: rusqlite::Result<Vec<NoteKey>> = stmt
            .query_map([], |row| Ok(NoteKey(row.get(2)?, row.get(1)?)))?
            .collect();

        labels
    }

    fn insert_label(&mut self, title: &str, repo_key: &str) -> rusqlite::Result<()> {
        let already_exists = self
            .get_labels()?
            .contains(&NoteKey(repo_key.to_string(), title.to_string()));

        if already_exists {
            return Ok(());
        }

        self.connection.execute(
            "INSERT INTO label (title, repo) VALUES (?1, ?2)",
            (title, repo_key),
        )?;

        Ok(())
    }

    pub fn register_note(mut self, note: &NoteSnapshot) -> rusqlite::Result<Self> {
        let NoteSnapshot {
            title, key, body, ..
        } = note.to_owned();

        let NoteKey(repo_key, _) = key;

        // Register this note.
        self.insert_label(&title, &repo_key)?;

        // Register outgoing links.
        let outgoing = MarkdownDocument::new(&body).get_links();
        for link in outgoing {
            if let LinkTarget::Note(title) = link.target {
                let note_key = title + ".md";
                self.insert_label(&note_key, &repo_key)?;
            }
        }

        Ok(self)
    }

    pub fn search(&self, query: &str) -> rusqlite::Result<Vec<SearchResult>> {
        let scorer = jaro_winkler::BatchComparator::new(query.to_lowercase().chars());

        let mut labels: Vec<SearchResult> = self
            .get_labels()?
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

        Ok(labels)
    }
}

#[cfg(test)]
mod tests {
    use crate::repository::MockRepository;

    use super::*;

    #[test]
    fn test_label_search() -> rusqlite::Result<()> {
        let mut repo = MockRepository::new();
        let graph = RepositoryGraph::new()?;

        assert_eq!(graph.search("note")?, []);

        let note = repo.insert_note("note.md", "This points to [[another note]]. Two links to make sure that it removes duplicates [[another note]].");
        let graph = graph.register_note(&note)?;

        let note_key = note.key;
        let another_note_key = NoteKey(note_key.clone().0, String::from("another note.md"));

        let search_results: Vec<NoteKey> = graph
            .search("note")?
            .iter()
            .map(|result| result.key.clone())
            .collect();
        assert_eq!(search_results, vec![note_key, another_note_key]);

        Ok(())
    }

    #[test]
    fn test_label_search_empty_query() -> rusqlite::Result<()> {
        let mut repo = MockRepository::new();
        let graph = RepositoryGraph::new()?;

        assert_eq!(graph.search("")?, []);

        let note = repo.insert_note("note.md", "This points to [[another note]]. Two links to make sure that it removes duplicates [[another note]].");
        let graph = graph.register_note(&note)?;

        let note_key: NoteKey = note.key;
        let another_note_key = NoteKey(note_key.clone().0, String::from("another note.md"));

        let search_results: Vec<NoteKey> = graph
            .search("")?
            .iter()
            .map(|result| result.key.clone())
            .collect();

        // Results should be in alphabetical order.
        assert_eq!(search_results, vec![another_note_key, note_key]);

        Ok(())
    }
}
