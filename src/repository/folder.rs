use crate::repository::{NoteKey, NoteSnapshot, Repository, UpstreamState};
use std::{
    fs::{self, DirEntry},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

pub type Result<T> = super::Result<T>;

/// A `Repository` where the notes are files from a local directory.
#[derive(Debug, PartialEq)]
pub struct FolderRepository {
    id: String,
    root: PathBuf,
}

impl FolderRepository {
    /// Creates a repository where the notes correspond to files on
    /// the given `root` path.
    pub fn new(root: &Path) -> Result<Self> {
        let root = fs::canonicalize(root)?;
        Ok(FolderRepository {
            id: title_from_path(&root)?.to_owned(),
            root,
        })
    }

    /// Utility to open a `FolderRepository` and possibly a note from a given
    /// path.
    ///
    /// If the path points to a folder, this will return `Ok((repo, None))`. If
    /// the path points to a file, this will return `Ok((repo, Some(note))` where
    /// `repo` is the first parent directory of `note`.
    pub fn open_path(path: &Path) -> io::Result<(Self, Option<NoteSnapshot>)> {
        let path = path.canonicalize()?;
        if path.is_file() {
            let repo_path = path
                .parent()
                .ok_or(io::Error::from(ErrorKind::NotADirectory))?;

            let repo = Self::new(repo_path)?;

            let note_id = title_from_path(&path)?.to_owned();
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

    /// Derives the path of a note in this repository from a note ID.
    fn get_note_path(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }
}

impl Repository for FolderRepository {
    fn note(&self, note_id: &str) -> io::Result<NoteSnapshot> {
        let full_path = self.get_note_path(note_id);
        let name = title_from_path(&full_path)?;
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
            upstream: UpstreamState::Exists,
            extension: extension.map(String::from),
        })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn edit_externally(&mut self, id: &str) -> io::Result<()> {
        edit::edit_file(self.get_note_path(id))
    }

    /// Returns all note keys in this repository.
    ///
    /// All files in this folder are considered notes, subdirectories
    /// excluded.
    ///
    /// The order is platform and filesystem dependent, consistent with `fs::read_dir`.
    ///
    /// Returns an `io::Error` if any file could not be read. Notes need UTF-8 note keys,
    /// which are determined from the file name. If this fails for any file, the function will
    /// return an error.
    fn notes(&self) -> io::Result<Vec<NoteKey>> {
        let root = &self.root.to_owned();

        let files: io::Result<Vec<DirEntry>> =
            fs::read_dir(root)?.try_fold(vec![], |mut file_list, entry| {
                let entry = entry?;
                if entry.file_type()?.is_file() {
                    file_list.push(entry);
                }
                Ok(file_list)
            });

        let notes: io::Result<Vec<NoteKey>> = files?
            .iter()
            .map(|entry| {
                let path = entry.path();
                let rel = path
                    .strip_prefix(root)
                    .expect("should be able to strip prefix from path");

                let id = rel.to_str();

                if let Some(id) = id {
                    Ok(self.note_key(id))
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::Other,
                        "Could not get UTF-8 name of file",
                    ))
                }
            })
            .collect();

        notes
    }

    /// Saves the note to a file in the directory of this repository.
    ///
    /// To prevent data loss from partial writes, it writes to a temporary
    /// file which is later renamed to the target file.
    fn commit_note(&mut self, note: NoteSnapshot) -> io::Result<()> {
        let NoteKey(repo_id, note_id) = note.key;
        // To prevent a note from another repo to be committed to this repo.
        assert_eq!(repo_id, self.id());

        let path = self.get_note_path(&note_id);

        let mut tempfile = NamedTempFile::new_in(&self.root)?;
        tempfile.write_all(note.body.as_bytes())?;

        // We want to wait for the file content to be saved to the temporary file before renaming it.
        tempfile.as_file().sync_all()?;
        tempfile.persist(path)?;

        Ok(())
    }
}

/// Derives a readable title of the note from the path.
fn title_from_path(file_path: &Path) -> Result<&str> {
    file_path
        .file_name()
        .and_then(|os_str| os_str.to_str())
        .ok_or(io::Error::new(ErrorKind::Other, "No readable name"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::set_current_dir;
    use tempfile::TempDir;

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
    fn test_title_from_path() -> io::Result<()> {
        let dir_path = PathBuf::from("points/to/directory");
        let file_path = PathBuf::from("points/to/file.md");

        assert_eq!(title_from_path(&dir_path)?, "directory");
        assert_eq!(title_from_path(&file_path)?, "file.md");

        Ok(())
    }

    #[test]
    fn test_new_repo() -> io::Result<()> {
        let (_tempdir, repo_path) = create_temp_repo_path("repository")?;
        fs::create_dir(repo_path.join("subfolder"))?;

        let expected_repo = FolderRepository {
            root: repo_path.clone(),
            id: String::from("repository"),
        };

        // It should work when outside the folder.
        assert_eq!(FolderRepository::new(&repo_path)?, expected_repo);

        // It should work when inside the folder.
        set_current_dir(&repo_path)?;
        assert_eq!(FolderRepository::new(&PathBuf::from("."))?, expected_repo);

        // It should work when inside a subdirectory of the folder
        set_current_dir(repo_path.join("subfolder"))?;
        assert_eq!(FolderRepository::new(&PathBuf::from(".."))?, expected_repo);

        Ok(())
    }

    #[test]
    fn test_open_path() -> io::Result<()> {
        let (_tempdir, repo_path) = create_temp_repo_path("repository")?;
        create_note(&repo_path, "note.md", "This is the content")?;

        let expected_repo = FolderRepository::new(&repo_path)?;
        let expected_note = expected_repo.note(&String::from("note.md"))?;

        // Path pointing to repo.
        let (repo, note) = FolderRepository::open_path(&repo_path)?;

        assert_eq!(repo, expected_repo);
        assert_eq!(note, None);

        // Path pointing to note in repo.
        let (repo, note) = FolderRepository::open_path(&repo_path.join("note.md"))?;

        assert_eq!(repo, expected_repo);
        assert_eq!(note, Some(expected_note.clone()));

        // Path pointing to note in repo, while inside repo.
        set_current_dir(repo_path)?;
        let (repo, note) = FolderRepository::open_path(Path::new("note.md"))?;

        assert_eq!(repo, expected_repo);
        assert_eq!(note, Some(expected_note.clone()));

        Ok(())
    }

    #[test]
    fn test_get_note() -> io::Result<()> {
        let (_tempdir, repo_path) = create_temp_repo_path("repository")?;
        create_note(&repo_path, "note.md", "This is the content")?;

        let repo = FolderRepository::new(&repo_path)?;

        assert_eq!(
            repo.note("note.md")?,
            NoteSnapshot {
                key: NoteKey::from(("repository", "note.md")),
                title: String::from("note.md"),
                body: String::from("This is the content"),
                upstream: UpstreamState::Exists,
                extension: Some(String::from("md"))
            }
        );

        assert_eq!(
            repo.note("new_note.md")?,
            NoteSnapshot {
                key: NoteKey::from(("repository", "new_note.md")),
                title: String::from("new_note.md"),
                body: String::new(),
                upstream: UpstreamState::New,
                extension: Some(String::from("md"))
            }
        );

        Ok(())
    }

    #[test]
    fn test_list_notes() -> io::Result<()> {
        let (_tempdir, repo_path) = create_temp_repo_path("repository")?;
        create_note(&repo_path, "Note A.md", "")?;
        create_note(&repo_path, "Note B.md", "")?;
        create_note(&repo_path, "Note C.md", "")?;
        fs::create_dir(repo_path.join("Not a note"))?;

        let repo = FolderRepository::new(&repo_path)?;

        let mut notes = repo.notes()?;
        // Order is not well-defined, sort before assertion.
        notes.sort();

        assert_eq!(
            notes,
            vec![
                repo.note_key("Note A.md"),
                repo.note_key("Note B.md"),
                repo.note_key("Note C.md"),
            ]
        );

        Ok(())
    }

    #[test]
    fn test_commit_note() -> io::Result<()> {
        let (_tempdir, repo_path) = create_temp_repo_path("repository")?;

        let mut repo = FolderRepository::new(&repo_path)?;

        let mut note = repo.note("new.md")?;
        note.body = String::from("This will become a new note.");

        repo.commit_note(note)?;

        assert_eq!(
            fs::read_to_string(repo_path.join("new.md"))?,
            "This will become a new note."
        );

        create_note(&repo_path, "existing.md", "Original text")?;
        let mut note = repo.note("existing.md")?;
        note.body = String::from("New text.");

        repo.commit_note(note)?;

        assert_eq!(
            fs::read_to_string(repo_path.join("existing.md"))?,
            "New text."
        );

        Ok(())
    }

    #[test]
    #[should_panic]
    fn test_commit_note_wrong_repo() {
        let (_tempdir_a, repo_path_a) = create_temp_repo_path("repo_a").unwrap();
        let (_tempdir_b, repo_path_b) = create_temp_repo_path("repo_b").unwrap();
        let mut repo_a = FolderRepository::new(&repo_path_a).unwrap();
        let repo_b = FolderRepository::new(&repo_path_b).unwrap();

        // This should panic as the note from repo b can not be committed to repo a.
        let _ = repo_a.commit_note(repo_b.note("note.md").unwrap());
    }
}
