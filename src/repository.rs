pub mod folder;
#[cfg(test)]
pub mod mock;

use std::io;

pub type Result<T> = io::Result<T>;

/// A collection of resources, called "notes", from a generic source, called "upstream".
pub trait Repository {
    fn id(&self) -> &str;

    /// Retrieves a snapshot of a note from upstream.
    fn note(&self, id: &str) -> Result<NoteSnapshot>;

    /// Retrieves the `NoteKey` of the note in this repository with given
    /// `note_id`.
    fn note_key(&self, note_id: &str) -> NoteKey {
        NoteKey(self.id().to_owned(), note_id.to_owned())
    }

    /// Lists all note keys of this repository.
    fn notes(&self) -> Result<Vec<NoteKey>>;

    /// Pushes the body of a `NoteSnapshot` upstream.
    ///
    /// After a successful commit, subsequent calls to `Repository::note`
    /// should be guaranteed to result in a `NoteSnapshot` with the same body.
    fn commit_note(&mut self, note: NoteSnapshot) -> Result<()>;

    /// Opens a note in an external application.
    fn edit_externally(&mut self, id: &str) -> Result<()>;
}

/// The state of a note upstream.
#[derive(Clone, Debug, PartialEq)]
pub enum UpstreamState {
    /// The note does not yet exist upstream.
    New,
    /// The note already exists upstream.
    Exists,
}

/// A snapshot in time of the body of a note along with metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteSnapshot {
    pub key: NoteKey,

    /// A readable title of the note.
    pub title: String,

    /// The content of the note.
    pub body: String,

    /// The state of the note upstream.
    pub upstream: UpstreamState,

    /// Optional extensions of the id.
    ///
    /// Corresponds to the file extensions that this note would have
    /// on a file system.
    pub extension: Option<String>,
}

impl NoteSnapshot {
    /// Creates an empty note that does not exist upstream.
    fn new_note(key: &NoteKey, extension: Option<&str>) -> Self {
        let NoteKey(_, id) = key;

        NoteSnapshot {
            key: key.to_owned(),
            title: id.to_owned(),
            body: String::new(),
            upstream: UpstreamState::New,
            extension: extension.map(String::from),
        }
    }
}

/// A unique handle to any version of a specific note in a specific repository.
///
/// All notes have a "note ID" which identifies the note within the repository
/// it belongs to. A `NoteKey` combines this with a "repo id" which identifies
/// the repository.
///
/// # Usage
/// `NoteKey(repo_id, note_id)`
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NoteKey(pub String, pub String);

impl From<(&str, &str)> for NoteKey {
    fn from(value: (&str, &str)) -> Self {
        let (repo_id, note_id) = value;
        NoteKey(repo_id.to_owned(), note_id.to_owned())
    }
}

/// Finds the note ID corresponding to a label.
///
/// For example, `note.md` is a note ID with label `note`.
pub fn resolve_label(label: &str) -> String {
    // TODO: Check if label has an extension before appending.
    // TODO: Move to graph.
    format!("{}.md", label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_label() {
        assert_eq!(resolve_label("note"), "note.md");
        assert_eq!(resolve_label("subfolder/note"), "subfolder/note.md");
    }
}
