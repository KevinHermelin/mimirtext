pub mod markdown;

pub trait Document {
    /// Returns the outgoing links of this document in the order that
    /// they appear when rendered.
    fn links(&self) -> Vec<LinkTarget>;
}

/// Destination for a link.
#[derive(Debug, PartialEq, Clone)]
pub enum LinkTarget {
    Note(String),
    External(String),
}
