use crate::repository::{NoteKey, NoteSnapshot, Repository, UpstreamState};
use std::collections::HashMap;
use std::io;
use uuid::Uuid;

/// A `Repository` used for tests. All notes are saved in memory.
pub struct MockRepository {
    notes: HashMap<String, NoteSnapshot>,
    id: String,
    pub edit_externally_impl: Box<dyn FnMut(NoteSnapshot) -> NoteSnapshot + Send + Sync>,
}

impl MockRepository {
    pub fn new() -> Self {
        MockRepository {
            notes: HashMap::new(),
            id: Uuid::new_v4().to_string(),
            edit_externally_impl: Box::new(|note| note),
        }
    }
    pub fn insert_note(&mut self, id: &str, content: &str) -> NoteSnapshot {
        let title = id.to_owned();
        let body = content.to_owned();
        let extension = id.split('.').next_back();

        let key = self.note_key(id);

        let note = NoteSnapshot {
            key,
            title,
            body,
            upstream: UpstreamState::Exists,
            extension: extension.map(String::from),
        };

        self.notes.insert(id.to_owned(), note.clone());

        note
    }
}

impl Repository for MockRepository {
    fn note(&self, id: &str) -> io::Result<NoteSnapshot> {
        let note_key = NoteKey(self.id().to_owned(), id.to_owned());
        let extension = id.split('.').next_back();

        let note = self
            .notes
            .get(id)
            .cloned()
            .unwrap_or(NoteSnapshot::new_note(&note_key, extension));
        Ok(note)
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn edit_externally(&mut self, id: &str) -> io::Result<()> {
        let note = self.note(id)?;
        let new_note = (self.edit_externally_impl)(note.to_owned());
        self.notes.insert(id.to_owned(), new_note);
        Ok(())
    }

    fn notes(&self) -> io::Result<Vec<NoteKey>> {
        Ok(self
            .notes
            .keys()
            .cloned()
            .map(|note| self.note_key(&note))
            .collect())
    }

    fn commit_note(&mut self, note: NoteSnapshot) -> io::Result<()> {
        self.insert_note(&note.key.1, &note.body);
        Ok(())
    }
}
