use pulldown_cmark::{Event, LinkType, Options, Parser, Tag};

use crate::document::{Document, LinkTarget};

#[derive(Debug, PartialEq)]
pub struct MarkdownDocument {
    content: String,
}

impl Document for MarkdownDocument {
    fn links(&self) -> Vec<LinkTarget> {
        self.parser()
            .filter_map(|event| match event {
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    ..
                }) => Some((link_type, dest_url.to_string())),
                _ => None,
            })
            .filter_map(|(link_type, dest_url)| match link_type {
                LinkType::WikiLink { .. } => Some(LinkTarget::Note(dest_url)),
                LinkType::Inline => Some(LinkTarget::External(dest_url)),
                _ => None,
            })
            .collect()
    }
}

impl MarkdownDocument {
    pub fn new(content: &str) -> Self {
        Self {
            content: content.to_owned(),
        }
    }
    pub fn parser(&self) -> Parser {
        let options = Options::ENABLE_WIKILINKS.union(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
        Parser::new_ext(&self.content, options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_links() {
        assert_eq!(
            MarkdownDocument::new(
                "[[This page]] has [[Link|links]], of [different](url.com) kinds."
            )
            .links(),
            vec![
                LinkTarget::Note(String::from("This page")),
                LinkTarget::Note(String::from("Link")),
                LinkTarget::External(String::from("url.com")),
            ]
        );

        assert_eq!(
            MarkdownDocument::new("This page has no links.").links(),
            vec![]
        );
    }
}
