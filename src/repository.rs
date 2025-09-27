use rapidfuzz::distance::jaro_winkler;
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
pub struct NoteKey(pub String, pub String);

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
    pub extension: Option<String>,
}

impl NoteSnapshot {
    fn new_note(key: &NoteKey, extension: Option<&str>) -> Self {
        let NoteKey(_, id) = key;

        NoteSnapshot {
            key: key.to_owned(),
            title: id.to_owned(),
            body: String::new(),
            state: NoteState::New,
            extension: extension.map(String::from),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub key: NoteKey,
    distance: f64,
}

impl SearchResult {
    pub fn new(key: NoteKey, distance: f64) -> Self {
        SearchResult { key, distance }
    }
}

pub trait Repository {
    fn id(&self) -> &str;
    fn resolve_reference(&self, reference: &str) -> String;
    fn note(&self, id: &str) -> io::Result<NoteSnapshot>;
    fn note_key(&self, note_id: &str) -> NoteKey {
        NoteKey(self.id().to_owned(), note_id.to_owned())
    }
    fn notes(&self) -> io::Result<Vec<NoteKey>>;
    fn search(&self, query: &str) -> io::Result<Vec<SearchResult>> {
        let notes = self.notes()?;

        let scorer = jaro_winkler::BatchComparator::new(query.to_lowercase().chars());

        let mut notes: Vec<SearchResult> = notes
            .iter()
            .map(|key| {
                SearchResult::new(key.clone(), scorer.distance(key.1.to_lowercase().chars()))
            })
            .collect();

        if query.is_empty() {
            notes.sort_by_key(|result| result.key.1.clone());
        } else {
            notes.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
        }

        Ok(notes)
    }
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
        let root = fs::canonicalize(root)?;
        Ok(FolderRepository {
            id: name_from_path(&root)?.to_owned(),
            root: root,
        })
    }
    pub fn open_path(path: &Path) -> io::Result<(Self, Option<NoteSnapshot>)> {
        let path = path.canonicalize()?;
        if path.is_file() {
            let repo_path = path
                .parent()
                .ok_or(io::Error::from(ErrorKind::NotADirectory))?;

            let repo = Self::new(repo_path)?;

            let note_id = name_from_path(&path)?.to_owned();
            let note = repo.note(&note_id)?;

            return Ok((repo, Some(note)));
        }
        if path.is_dir() {
            let repo = Self::new(&path)?;
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
        let extension = full_path
            .extension()
            .and_then(|extension| extension.to_str());

        let key = self.note_key(note_id);
        let title = name.to_owned();

        if !fs::exists(&full_path)? {
            return Ok(NoteSnapshot::new_note(&key, extension));
        }

        let body = fs::read_to_string(&full_path)?;
        Ok(NoteSnapshot {
            key,
            title,
            body,
            state: NoteState::Exists,
            extension: extension.map(String::from),
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn edit_externally(&mut self, id: &str) -> io::Result<()> {
        edit::edit_file(self.get_note_path(id))
    }

    fn notes(&self) -> io::Result<Vec<NoteKey>> {
        let root = &self.root.to_owned();

        // There are multiple reasons why a note can not be read.
        // This implementation will simply filter out all such notes.
        // One can argue that it will not matter as long as this function
        // is only used for searching.

        Ok(fs::read_dir(root)?
            .filter_map(|res| res.ok())
            .filter_map(|entry| {
                let path = entry.path();
                let rel = path.strip_prefix(root).ok()?;
                rel.to_str().map(|id| self.note_key(id))
            })
            .collect())
    }
}

#[cfg(test)]
pub struct MockRepository {
    notes: HashMap<String, NoteSnapshot>,
    id: String,
    pub edit_externally_impl: Box<dyn FnMut(NoteSnapshot) -> NoteSnapshot>,
}

#[cfg(test)]
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
        let extension = id.split('.').last();

        let key = self.note_key(&id);

        let note = NoteSnapshot {
            key,
            title,
            body,
            state: NoteState::Exists,
            extension: extension.map(String::from),
        };

        self.notes.insert(id.to_owned(), note.clone());

        note
    }
}

#[cfg(test)]
impl Repository for MockRepository {
    fn resolve_reference(&self, reference: &str) -> String {
        format!("{}.md", reference)
    }

    fn note(&self, id: &str) -> io::Result<NoteSnapshot> {
        let note_key = NoteKey(self.id().to_owned(), id.to_owned());
        let extension = id.split('.').last();

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
}

#[cfg(test)]
mod tests {
    use std::env::set_current_dir;
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
                root: repo_path.clone(),
                id: String::from("repository")
            }
        );

        // It should also work if the current directory is inside the repository.
        set_current_dir(&repo_path)?;

        let repo = FolderRepository::new(&PathBuf::from("."))?;

        assert_eq!(
            repo,
            FolderRepository {
                root: repo_path.clone(),
                id: String::from("repository")
            }
        );

        // Or if in a subfolder of the repository, using "..".
        fs::create_dir("subfolder")?;
        set_current_dir(&repo_path.join("subfolder"))?;

        let repo = FolderRepository::new(&PathBuf::from(".."))?;

        assert_eq!(
            repo,
            FolderRepository {
                root: repo_path.clone(),
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
                state: NoteState::Exists,
                extension: Some(String::from("md"))
            }
        );

        assert_eq!(
            repo.note(&String::from("new_note.md"))?,
            NoteSnapshot {
                key: NoteKey::from(("repository", "new_note.md")),
                title: String::from("new_note.md"),
                body: String::new(),
                state: NoteState::New,
                extension: Some(String::from("md"))
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
        assert_eq!(note, Some(expected_note.clone()));

        // It should also work if pointing to a file inside the current directory.
        set_current_dir(repo_path)?;
        let (repo, note) = FolderRepository::open_path(&Path::new("note.md"))?;

        assert_eq!(repo, expected_repo);
        assert_eq!(note, Some(expected_note.clone()));

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

    #[test]
    fn test_folder_repository_search() -> io::Result<()> {
        let (_tempdir, repo_path) = create_temp_repo_path("repository")?;
        create_note(&repo_path, "This is a note.md", "")?;
        create_note(&repo_path, "This is something else.md", "")?;
        create_note(&repo_path, "Another thing.md", "")?;
        create_note(&repo_path, "This is note.md", "")?;

        let repo = FolderRepository::new(&repo_path)?;

        let results: Vec<String> = repo
            .search("This is an note")?
            .iter()
            .map(|result| result.key.1.clone())
            .collect();
        assert_eq!(
            results,
            [
                String::from("This is a note.md"),
                String::from("This is note.md"),
                String::from("This is something else.md"),
                String::from("Another thing.md"),
            ]
        );

        Ok(())
    }

    #[test]
    fn test_folder_repository_search_empty_query() -> io::Result<()> {
        let (_tempdir, repo_path) = create_temp_repo_path("repository")?;
        create_note(&repo_path, "ABC.md", "")?;
        create_note(&repo_path, "DEF.md", "")?;
        create_note(&repo_path, "GHI.md", "")?;
        create_note(&repo_path, "JKL.md", "")?;

        let repo = FolderRepository::new(&repo_path)?;

        let results: Vec<String> = repo
            .search("")?
            .iter()
            .map(|result| result.key.1.clone())
            .collect();
        assert_eq!(
            results,
            [
                String::from("ABC.md"),
                String::from("DEF.md"),
                String::from("GHI.md"),
                String::from("JKL.md"),
            ]
        );

        Ok(())
    }
}
