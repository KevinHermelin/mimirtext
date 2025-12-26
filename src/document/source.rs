use crate::{
    document::{Document, LinkTarget},
    repository::NoteSnapshot,
};

#[derive(Debug, PartialEq)]
pub struct SourceDocument {
    pub content: String,
}

impl SourceDocument {
    pub fn new(content: &str) -> Self {
        Self {
            content: content.to_owned(),
        }
    }
}

impl Document for SourceDocument {
    fn links(&self) -> Vec<LinkTarget> {
        vec![]
    }

    fn try_new(note: &NoteSnapshot) -> Option<Self> {
        Some(Self::new(&note.body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::mock::MockRepository;

    #[test]
    fn test_try_from() {
        let mut repo = MockRepository::new();

        let some_extension_note = repo.insert_note("some extension.html", "Some note");
        let no_extension_note = repo.insert_note("no extension", "Some note");

        assert_eq!(
            SourceDocument::try_new(&some_extension_note),
            Some(SourceDocument {
                content: String::from("Some note")
            })
        );
        assert_eq!(
            SourceDocument::try_new(&no_extension_note),
            Some(SourceDocument {
                content: String::from("Some note")
            })
        );
    }

    #[test]
    fn test_get_links() {
        assert_eq!(
            SourceDocument::new("Being a [[Source]] document, it has no links.").links(),
            vec![]
        );
    }
}
