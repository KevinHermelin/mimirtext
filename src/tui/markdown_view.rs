use crate::{document::markdown::MarkdownDocument, model::note_pane::NoteContext};
use pulldown_cmark::{Event, Tag, TagEnd};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};

fn flush_line<'a>(lines: &mut Vec<Line<'a>>, line: &mut Vec<Span<'a>>) {
    lines.push(Line::from_iter(line.clone()));
    line.clear();
}

impl MarkdownDocument {
    fn get_lines(&self, context: &NoteContext) -> Vec<Line> {
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

        for event in self.parser() {
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
                        if context.link_selection_index == link_index {
                            style = style.reversed();
                        }
                    }
                    // There might be newlines in the text.
                    for (i, part) in text.split('\n').enumerate() {
                        // First line in this text event might be a continuation of a previous text event.
                        if i > 0 {
                            flush_line(&mut lines, &mut line);
                        }
                        line.push(Span::styled(part.to_owned(), style));
                    }
                }
                Event::SoftBreak => {
                    flush_line(&mut lines, &mut line);
                }
                Event::HardBreak => {
                    flush_line(&mut lines, &mut line);
                }
                Event::Start(tag) => match tag {
                    Tag::Heading { .. } => {
                        if !lines.is_empty() || !line.is_empty() {
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
}

pub struct MarkdownView {
    document: MarkdownDocument,
    context: NoteContext,
}

impl MarkdownView {
    pub fn new(document: MarkdownDocument, context: NoteContext) -> Self {
        MarkdownView { document, context }
    }
}

impl Widget for &MarkdownView {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(self.document.get_lines(&self.context))
            .wrap(Wrap { trim: false })
            .scroll((self.context.scroll_lines.try_into().unwrap(), 0))
            .render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use crate::repository::mock::MockRepository;

    use super::*;
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn test_render_view() {
        let mut repo = MockRepository::new();
        let note = repo.insert_note("Note", "
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
        ");

        let view = MarkdownView::new(MarkdownDocument::new(&note.body), NoteContext::new(note));
        let mut terminal = Terminal::new(TestBackend::new(80, 22)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&view, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());
    }
}
