use std::fs::File;
use std::io::{Read, Result};
use std::path::{Path, PathBuf};

pub struct FileBuffer {
    pub file_path: PathBuf,
    pub content: String,
    pub created: bool,
}

impl FileBuffer {
    pub fn from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(FileBuffer {
                file_path: path.to_path_buf(),
                content: String::from(""),
                created: false,
            });
        }
        let mut file = File::open(path)?;

        let mut content = String::new();
        file.read_to_string(&mut content)?;

        Ok(FileBuffer {
            file_path: path.to_path_buf(),
            content: content,
            created: true,
        })
    }
}

#[cfg(test)]
impl FileBuffer {
    pub fn mock(content: &str) -> Self {
        FileBuffer {
            content: content.to_owned(),
            file_path: PathBuf::from("mock.txt"),
            created: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_from_file() {
        let mut file = NamedTempFile::new().expect("should be able to create tempfile");
        let content = "This is a test file";

        write!(file, "{content}").expect("should be able to write to tempfile");
        let buffer = FileBuffer::from_file(file.path()).expect("should be able to read tempfile");

        assert_eq!(buffer.content, content);
        assert_eq!(buffer.file_path, file.path());
    }

    #[test]
    fn test_from_file_non_existent() {
        let path = PathBuf::from("nonexistent.md");
        let buffer = FileBuffer::from_file(&path)
            .expect("should be able to create FileBuffer for non-existent file");

        assert_eq!(buffer.content, "");
        assert_eq!(buffer.file_path, path);
        assert_eq!(buffer.created, false);
    }
}
