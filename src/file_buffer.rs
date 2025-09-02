use std::fs::File;
use std::io::{Read, Result};
use std::path::{Path, PathBuf};

pub struct FileBuffer {
    pub file_path: PathBuf,
    pub content: String,
}

impl FileBuffer {
    pub fn from_file(path: &Path) -> Result<Self> {
        let mut file = File::open(path)?;

        let mut content = String::new();
        file.read_to_string(&mut content)?;

        Ok(FileBuffer {
            file_path: path.to_path_buf(),
            content: content,
        })
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
}
