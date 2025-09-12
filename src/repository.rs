#[cfg(test)]
use std::collections::HashMap;
use std::{
    io,
    path::{Path, PathBuf},
};

use crate::file_buffer::FileBuffer;

pub trait Note {
    fn content(&self) -> &str;
    fn name(&self) -> &str;
    fn id(&self) -> &str;
    fn edit_externally(&mut self) -> io::Result<()>;
}

pub trait Repository {
    fn resolve_reference(&self, reference: &str) -> String;
    fn get_note(&mut self, id: &str) -> io::Result<Box<dyn Note>>;
}

pub struct FileNote {
    note_name: String,
    note_id: String,
    file_buffer: FileBuffer,
}

fn name_from_path(file_path: &Path) -> Option<&str> {
    file_path.file_name().and_then(|os_str| os_str.to_str())
}

impl FileNote {
    fn new(id: &str, file_buffer: FileBuffer) -> Self {
        let name =
            name_from_path(&file_buffer.file_path).expect("note should have a readable name");

        FileNote {
            note_name: name.to_owned(),
            note_id: id.to_owned(),
            file_buffer,
        }
    }
}

impl Note for FileNote {
    fn content(&self) -> &str {
        &self.file_buffer.content
    }

    fn name(&self) -> &str {
        &self.note_name
    }

    fn id(&self) -> &str {
        &self.note_id
    }

    fn edit_externally(&mut self) -> io::Result<()> {
        edit::edit_file(&self.file_buffer.file_path)?;
        self.file_buffer = FileBuffer::from_file(&self.file_buffer.file_path)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct FolderRepository {
    root: PathBuf,
}

impl FolderRepository {
    pub fn open_path(path: &Path) -> (Option<Self>, Option<FileNote>) {
        if path.is_file() {
            let repo = path
                .parent()
                .filter(|path| path.is_dir())
                .map(|path| FolderRepository {
                    root: path.to_owned(),
                });

            let id =
                name_from_path(&path).expect("path should point to file with readable file name");
            let note = FileBuffer::from_file(path)
                .ok()
                .map(|file_buffer| FileNote::new(id, file_buffer));

            return (repo, note);
        }
        if path.is_dir() {
            let repo = Some(FolderRepository {
                root: path.to_owned(),
            });
            return (repo, None);
        }
        (None, None)
    }
}

impl Repository for FolderRepository {
    fn resolve_reference(&self, reference: &str) -> String {
        format!("{}.md", reference)
    }

    fn get_note(&mut self, id: &str) -> io::Result<Box<dyn Note>> {
        let full_path = self.root.join(id);

        let file_buffer = FileBuffer::from_file(&full_path)?;
        let note = FileNote::new(id, file_buffer);
        Ok(Box::new(note))
    }
}

#[cfg(test)]
#[derive(Clone)]
pub struct InMemoryNote {
    note_content: String,
    note_name: String,
    note_id: String,
}

#[cfg(test)]
impl Note for InMemoryNote {
    fn content(&self) -> &str {
        &self.note_content
    }

    fn name(&self) -> &str {
        &self.note_name
    }

    fn id(&self) -> &str {
        &self.note_id
    }

    fn edit_externally(&mut self) -> io::Result<()> {
        todo!()
    }
}

#[cfg(test)]
#[derive(Default)]
pub struct InMemoryRepository {
    notes: HashMap<String, InMemoryNote>,
}

#[cfg(test)]
impl InMemoryRepository {
    pub fn insert_note(&mut self, id: &str, content: &str) -> Box<InMemoryNote> {
        let name = id;

        let note = InMemoryNote {
            note_name: name.to_owned(),
            note_content: content.to_owned(),
            note_id: id.to_owned(),
        };

        self.notes.insert(id.to_owned(), note.to_owned());

        Box::new(note)
    }
}

#[cfg(test)]
impl Repository for InMemoryRepository {
    fn resolve_reference(&self, reference: &str) -> String {
        format!("{}.md", reference)
    }

    fn get_note(&mut self, id: &str) -> io::Result<Box<dyn Note>> {
        let note = self
            .notes
            .get(id)
            .cloned()
            .map(Box::new)
            .unwrap_or_else(|| self.insert_note(id, ""));
        Ok(note)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::{fs::File, io::Write};
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_file_note_properties() {
        let file_note = FileNote::new(
            "text.md",
            FileBuffer {
                file_path: PathBuf::from("text.md"),
                content: String::from("It has content"),
            },
        );

        assert_eq!(file_note.content(), "It has content");
        assert_eq!(file_note.name(), "text.md");
        assert_eq!(file_note.id(), "text.md");
    }

    #[test]
    fn test_folder_repository_open_path() -> Result<()> {
        let directory = TempDir::new()?;
        let directory_path = directory.path();

        let note_path = directory_path.join("note.md");

        let mut note_file = File::create(&note_path)?;
        write!(note_file, "This is the content")?;

        let (repo, note) = FolderRepository::open_path(directory_path);
        assert!(repo.is_some_and(|repo| repo.root == directory.path()));
        assert!(note.is_none());

        let (repo, note) = FolderRepository::open_path(&directory_path.join("note.md"));
        assert!(repo.is_some_and(|repo| repo.root == directory.path()));
        assert!(note.is_some_and(|note| note.name() == "note.md"
            && note.content() == "This is the content"
            && note.id() == "note.md"));

        // There are two additional possibilities, repo could be missing while note exists and both could be missing.

        Ok(())
    }

    #[test]
    fn test_folder_repository_resolve_path() {
        let folder_repo = FolderRepository {
            root: PathBuf::new(),
        };

        assert_eq!(folder_repo.resolve_reference("note"), "note.md");
        assert_eq!(
            folder_repo.resolve_reference("subfolder/another_note"),
            "subfolder/another_note.md"
        );
    }

    #[test]
    fn test_folder_repository_get_note() -> Result<()> {
        let directory = TempDir::new()?;
        let mut note_file = File::create(directory.path().join("note.md"))?;
        write!(note_file, "This is the content")?;

        let (folder_repo, _) = FolderRepository::open_path(directory.path());
        let mut folder_repo = folder_repo.expect("should be a valid folder repo");

        let note = folder_repo
            .get_note("note.md")
            .expect("should be able to get note");

        assert_eq!(note.name(), "note.md");
        assert_eq!(note.content(), "This is the content");
        assert_eq!(note.id(), "note.md");
        Ok(())
    }

    #[test]
    fn test_folder_repository_get_new_note() -> Result<()> {
        let directory = TempDir::new()?;

        let (folder_repo, _) = FolderRepository::open_path(directory.path());
        let mut folder_repo = folder_repo.expect("should be a valid folder repo");

        let note = folder_repo
            .get_note("non-existent note.md")
            .expect("should be able to get note");
        assert_eq!(note.name(), "non-existent note.md");
        assert_eq!(note.content(), "");
        assert_eq!(note.id(), "non-existent note.md");
        Ok(())
    }
}
