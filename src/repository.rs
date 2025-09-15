#[cfg(test)]
use std::collections::HashMap;
use std::{
    fs::{self},
    io::{self, ErrorKind},
    path::{Path, PathBuf},
};
#[cfg(test)]
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct NoteKey(String, String);

impl NoteKey {
    pub fn note_id(&self) -> &str {
        &self.1
    }
}

impl From<(&str, &str)> for NoteKey {
    fn from(value: (&str, &str)) -> Self {
        let (repo_id, note_id) = value;
        NoteKey(repo_id.to_owned(), note_id.to_owned())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum NoteState {
    New,
    Exists,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NoteSnapshot {
    pub key: NoteKey,
    pub title: String,
    pub body: String,
    pub state: NoteState,
}

impl NoteSnapshot {
    fn new_note(key: &NoteKey) -> Self {
        let NoteKey(_, id) = key;

        NoteSnapshot {
            key: key.to_owned(),
            title: id.to_owned(),
            body: String::new(),
            state: NoteState::New,
        }
    }
}

pub trait Repository {
    fn id(&self) -> &str;
    fn resolve_reference(&self, reference: &str) -> String;
    fn note(&self, id: &str) -> io::Result<NoteSnapshot>;
    fn edit_externally(&mut self, id: &str) -> io::Result<()>;
}

fn name_from_path(file_path: &Path) -> io::Result<&str> {
    file_path
        .file_name()
        .and_then(|os_str| os_str.to_str())
        .ok_or(io::Error::new(ErrorKind::Other, "No readable name"))
}

#[derive(Debug, PartialEq)]
pub struct FolderRepository {
    id: String,
    root: PathBuf,
}

impl FolderRepository {
    pub fn new(root: &Path) -> io::Result<Self> {
        Ok(FolderRepository {
            root: root.to_owned(),
            id: name_from_path(root)?.to_owned(),
        })
    }
    pub fn open_path(path: &Path) -> io::Result<(Self, Option<NoteSnapshot>)> {
        if path.is_file() {
            let repo_path = path
                .parent()
                .ok_or(io::Error::from(ErrorKind::NotADirectory))?;

            let repo = Self::new(repo_path)?;

            let note_id = name_from_path(path)?.to_owned();
            let note = repo.note(&note_id)?;

            return Ok((repo, Some(note)));
        }
        if path.is_dir() {
            let repo = Self::new(path)?;
            return Ok((repo, None));
        }
        Err(io::Error::new(
            io::ErrorKind::Other,
            "Path is neither a note nor a repository",
        ))
    }
    fn get_note_path(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }
}

impl Repository for FolderRepository {
    fn resolve_reference(&self, reference: &str) -> String {
        format!("{}.md", reference)
    }

    fn note(&self, note_id: &str) -> io::Result<NoteSnapshot> {
        let full_path = self.get_note_path(note_id);
        let name = name_from_path(&full_path)?;

        let key = NoteKey(self.id().to_owned(), name.to_owned());
        let title = name.to_owned();

        if !fs::exists(&full_path)? {
            return Ok(NoteSnapshot::new_note(&key));
        }

        let body = fs::read_to_string(&full_path)?;
        Ok(NoteSnapshot {
            key,
            title,
            body,
            state: NoteState::Exists,
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn edit_externally(&mut self, id: &str) -> io::Result<()> {
        edit::edit_file(self.get_note_path(id))
    }
}

#[cfg(test)]
pub struct InMemoryRepository {
    notes: HashMap<String, NoteSnapshot>,
    id: String,
}

#[cfg(test)]
impl InMemoryRepository {
    pub fn new() -> Self {
        InMemoryRepository {
            notes: HashMap::new(),
            id: Uuid::new_v4().to_string(),
        }
    }
    pub fn insert_note(&mut self, id: &str, content: &str) -> NoteSnapshot {
        let title = id.to_owned();
        let id = id.to_owned();
        let body = content.to_owned();

        let key = NoteKey(self.id().to_owned(), id.clone());

        let note = NoteSnapshot {
            key,
            title,
            body,
            state: NoteState::Exists,
        };

        self.notes.insert(id, note.clone());

        note
    }
}

#[cfg(test)]
impl Repository for InMemoryRepository {
    fn resolve_reference(&self, reference: &str) -> String {
        format!("{}.md", reference)
    }

    fn note(&self, id: &str) -> io::Result<NoteSnapshot> {
        let note_key = NoteKey(self.id().to_owned(), id.to_owned());
        let note = self
            .notes
            .get(id)
            .cloned()
            .unwrap_or(NoteSnapshot::new_note(&note_key));
        Ok(note)
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn edit_externally(&mut self, _: &str) -> io::Result<()> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn create_temp_repo_path(name: &str) -> io::Result<(TempDir, PathBuf)> {
        let temp_dir = TempDir::new()?;
        let repo_dir_path = temp_dir.path().join(name);
        fs::create_dir(&repo_dir_path)?;

        Ok((temp_dir, repo_dir_path))
    }

    fn create_note(repo_path: &Path, name: &str, content: &str) -> io::Result<PathBuf> {
        let note_path = repo_path.join(name);
        fs::write(&note_path, content)?;

        Ok(note_path)
    }

    #[test]
    fn test_note_key_getters() {
        assert_eq!(
            NoteKey(String::from("repo_b"), String::from("note_b")).note_id(),
            "note_b"
        );
    }

    #[test]
    fn test_name_from_path() -> io::Result<()> {
        let dir_path = PathBuf::from("points/to/directory");
        let file_path = PathBuf::from("points/to/file.md");

        assert_eq!(name_from_path(&dir_path)?, "directory");
        assert_eq!(name_from_path(&file_path)?, "file.md");

        Ok(())
    }

    #[test]
    fn test_folder_repository_new() -> io::Result<()> {
        let (_tempdir, repo_path) = create_temp_repo_path("repository")?;
        let repo = FolderRepository::new(&repo_path)?;

        assert_eq!(
            repo,
            FolderRepository {
                root: repo_path,
                id: String::from("repository")
            }
        );

        Ok(())
    }

    #[test]
    fn test_folder_repository_get_note() -> io::Result<()> {
        let (_tempdir, repo_path) = create_temp_repo_path("repository")?;
        create_note(&repo_path, "note.md", "This is the content")?;

        let repo = FolderRepository::new(&repo_path)?;

        assert_eq!(
            repo.note(&String::from("note.md"))?,
            NoteSnapshot {
                key: NoteKey::from(("repository", "note.md")),
                title: String::from("note.md"),
                body: String::from("This is the content"),
                state: NoteState::Exists
            }
        );

        assert_eq!(
            repo.note(&String::from("new_note.md"))?,
            NoteSnapshot {
                key: NoteKey::from(("repository", "new_note.md")),
                title: String::from("new_note.md"),
                body: String::new(),
                state: NoteState::New
            }
        );

        Ok(())
    }

    #[test]
    fn test_folder_repository_open_path() -> io::Result<()> {
        let (_tempdir, repo_path) = create_temp_repo_path("repository")?;
        create_note(&repo_path, "note.md", "This is the content")?;

        let expected_repo = FolderRepository::new(&repo_path)?;
        let expected_note = expected_repo.note(&String::from("note.md"))?;

        let (repo, note) = FolderRepository::open_path(&repo_path)?;

        assert_eq!(repo, expected_repo);
        assert_eq!(note, None);

        let (repo, note) = FolderRepository::open_path(&repo_path.join("note.md"))?;

        assert_eq!(repo, expected_repo);
        assert_eq!(note, Some(expected_note));

        Ok(())
    }

    #[test]
    fn test_folder_repository_resolve_reference() -> io::Result<()> {
        let (_tempdir, repo_path) = create_temp_repo_path("repository")?;
        create_note(&repo_path, "note.md", "This is the content")?;

        let repo = FolderRepository::new(&repo_path)?;

        assert_eq!(repo.resolve_reference("note"), "note.md");
        assert_eq!(
            repo.resolve_reference("subfolder/another_note"),
            "subfolder/another_note.md"
        );

        Ok(())
    }
}
