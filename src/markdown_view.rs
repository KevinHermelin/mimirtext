use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};

#[derive(Debug, PartialEq, Clone)]
pub enum LinkTarget {
    Note(String),
    External(String),
}

#[derive(Debug, PartialEq, Clone)]
pub struct LinkRef {
    index: usize,
    pub target: LinkTarget,
}

pub struct MarkdownDocument {
    content: String,
    pub selected_link: Option<LinkRef>,
}

fn flush_line<'a>(lines: &mut Vec<Line<'a>>, line: &mut Vec<Span<'a>>) {
    lines.push(Line::from_iter(line.clone()));
    line.clear();
}

impl MarkdownDocument {
    pub fn new(content: &str) -> Self {
        MarkdownDocument {
            content: content.to_owned(),
            selected_link: None,
        }
    }
    fn get_parser(&self) -> Parser {
        let options = Options::ENABLE_WIKILINKS.union(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
        Parser::new_ext(&self.content, options)
    }
    fn get_lines(&self) -> Vec<Line> {
        let mut lines = vec![];
        let mut line = vec![];

        let mut link_index = 0;

        let mut inside_heading = false;
        let mut inside_strong = false;
        let mut inside_emphasis = false;
        let mut inside_link = false;
        let mut inside_metadata = false;
        let mut list_level = 0;

        let mut last_tag: Option<TagEnd> = None;

        for event in self.get_parser() {
            match event {
                Event::Text(text) => {
                    let mut style = Style::new();
                    if inside_metadata {
                        continue;
                    }
                    if inside_emphasis {
                        style = style.italic();
                    }
                    if inside_strong {
                        style = style.bold();
                    }
                    if inside_heading {
                        style = style.bold().light_blue().underlined();
                    }
                    if inside_link {
                        style = style.cyan();
                        if self
                            .selected_link
                            .as_ref()
                            .is_some_and(|LinkRef { index, .. }| *index == link_index)
                        {
                            style = style.reversed();
                        }
                    }
                    line.push(Span::styled(text, style));
                }
                Event::Start(tag) => match tag {
                    Tag::Heading { .. } => {
                        if lines.len() != 0 || line.len() != 0 {
                            flush_line(&mut lines, &mut line);
                        }
                        inside_heading = true;
                    }
                    Tag::Paragraph => {
                        // Paragraphs should have spacing between.
                        if let Some(TagEnd::Paragraph) = last_tag {
                            flush_line(&mut lines, &mut line);
                        }
                    }
                    Tag::Strong => {
                        inside_strong = true;
                    }
                    Tag::Emphasis => {
                        inside_emphasis = true;
                    }
                    Tag::MetadataBlock(_) => {
                        inside_metadata = true;
                    }
                    Tag::List(_) => {
                        // If this list is indented, this new list will be embedded into an item tag.
                        // We need to flush the old item tag if this is the case.
                        if !line.is_empty() {
                            flush_line(&mut lines, &mut line);
                        }
                        list_level += 1;
                    }
                    Tag::Item => {
                        let text_element = Span::raw("  ".repeat(list_level - 1) + "• ");
                        line.push(text_element);
                    }
                    Tag::Link { .. } => {
                        inside_link = true;
                    }
                    _ => {}
                },
                Event::End(tag) => {
                    match tag {
                        TagEnd::Heading(_) => {
                            flush_line(&mut lines, &mut line);
                            flush_line(&mut lines, &mut line);
                            inside_heading = false;
                        }
                        TagEnd::Paragraph => {
                            flush_line(&mut lines, &mut line);
                        }
                        TagEnd::MetadataBlock(_) => {
                            inside_metadata = false;
                        }
                        TagEnd::Strong => {
                            inside_strong = false;
                        }
                        TagEnd::Emphasis => {
                            inside_emphasis = false;
                        }
                        TagEnd::List(_) => list_level -= 1,
                        TagEnd::Item => {
                            // It is possible that this particular item has already been flushed. See
                            // how List tags are entered.
                            if !line.is_empty() {
                                flush_line(&mut lines, &mut line);
                            }
                        }
                        TagEnd::Link => {
                            link_index += 1;
                            inside_link = false;
                        }
                        _ => {}
                    }
                    last_tag = Some(tag);
                }
                _ => {}
            }
        }
        flush_line(&mut lines, &mut line);
        lines
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

pub struct MarkdownView {
    document: MarkdownDocument,
    scroll_lines: i16,
}

impl MarkdownView {
    pub fn new(document: MarkdownDocument) -> Self {
        MarkdownView {
            document,
            scroll_lines: 0,
        }
    }
    pub fn scroll(mut self, scroll_lines: i16) -> Self {
        self.scroll_lines = scroll_lines;
        self
    }
}

impl Widget for &MarkdownView {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.document.get_lines())
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_lines.try_into().unwrap(), 0))
            .render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn test_render_view() {
        let view = MarkdownView::new(
            MarkdownDocument::new("
---
info: \"This metadata block should not be visible.\"
---
# This is a heading
This **is** a *paragraph* of text.
- This is a list
- with
- multiple
- items

## Another heading

This is another paragraph of text. It is long enough to be wrapped, yet every word in this paragraph should be visible. Additionally, this paragraph...

...and this paragraph should have spacing between.

- This is a list
\t- with multiple
\t\t- levels.
\t\t- This should be on level 2.
\t- This should be on level 1.
- This should be on level 0.
- This should be the last item.

[[This paragraph]] has a few [[Link|links]], which can be [selected](https://en.wikipedia.org/wiki/Selection_(user_interface)).

This should not be visible.
        "
        ));
        let mut terminal = Terminal::new(TestBackend::new(80, 22)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&view, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

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
