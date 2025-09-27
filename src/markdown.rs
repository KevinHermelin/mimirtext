use pulldown_cmark::{Event, LinkType, Options, Parser, Tag};

#[derive(Debug, PartialEq, Clone)]
pub enum LinkTarget {
    Note(String),
    External(String),
}

#[derive(Debug, PartialEq, Clone)]
pub struct LinkRef {
    pub index: usize,
    pub target: LinkTarget,
}

#[derive(Debug, PartialEq)]
pub struct MarkdownDocument {
    content: String,
    pub selected_link: Option<LinkRef>,
}

impl MarkdownDocument {
    pub fn new(content: &str) -> Self {
        MarkdownDocument {
            content: content.to_owned(),
            selected_link: None,
        }
    }
    pub fn get_parser(&self) -> Parser {
        let options = Options::ENABLE_WIKILINKS.union(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
        Parser::new_ext(&self.content, options)
    }
    pub fn get_links(&self) -> Vec<LinkRef> {
        self.get_parser()
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
                LinkType::Inline { .. } => Some(LinkTarget::External(dest_url)),
                _ => None,
            })
            .enumerate()
            .map(|(index, target)| LinkRef { index, target })
            .collect()
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
            .get_links(),
            vec![
                LinkRef {
                    index: 0,
                    target: LinkTarget::Note(String::from("This page"))
                },
                LinkRef {
                    index: 1,
                    target: LinkTarget::Note(String::from("Link"))
                },
                LinkRef {
                    index: 2,
                    target: LinkTarget::External(String::from("url.com"))
                },
            ]
        );

        assert_eq!(
            MarkdownDocument::new("This page has no links.").get_links(),
            vec![]
        );
    }
}
