use crate::{
    document::{markdown::MarkdownDocument, source::SourceDocument},
    repository::NoteSnapshot,
};

pub mod markdown;
pub mod source;

pub trait Document {
    /// Returns the outgoing links of this document in the order that
    /// they appear when rendered.
    fn links(&self) -> Vec<LinkTarget>;

    /// Attempts to create a document from a note snapshot.
    ///
    /// Returns `Some(document)` if this is possible, otherwise `None`.
    fn try_new(note: &NoteSnapshot) -> Option<Self>
    where
        Self: Sized;
}

/// Destination for a link.
#[derive(Debug, PartialEq, Clone)]
pub enum LinkTarget {
    Note(String),
    External(String),
}

#[derive(Debug, PartialEq)]
pub enum DocumentType {
    Markdown(MarkdownDocument),
    Source(SourceDocument),
}

impl DocumentType {
    pub fn as_document(&self) -> &dyn Document {
        match self {
            Self::Markdown(document) => document,
            Self::Source(document) => document,
        }
    }
}

impl NoteSnapshot {
    /// Creates a `Document` from a note snapshot and wraps it in a `DocumentType`.
    ///
    /// If the note is accepted as a `MarkdownDocument` (see `MarkdownDocument::try_new`),
    /// it returns a `DocumentType::Markdown(MarkdownDocument)`. Otheriwse this function
    /// returns a `DocumentType::Source(SourceDocument)`.
    pub fn parse(&self) -> DocumentType {
        if let Some(document) = MarkdownDocument::try_new(self) {
            return DocumentType::Markdown(document);
        }

        DocumentType::Source(SourceDocument::try_new(self).unwrap())
    }
}
