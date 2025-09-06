use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};

pub struct MarkdownView {
    content: String,
    scroll_lines: i16,
}

impl MarkdownView {
    pub fn new(content: &str) -> Self {
        MarkdownView {
            content: content.to_owned(),
            scroll_lines: 0,
        }
    }
    pub fn scroll(mut self, scroll_lines: i16) -> Self {
        self.scroll_lines = scroll_lines;
        self
    }
}

fn flush_line<'a>(paragraph: &mut Vec<Line<'a>>, line: &mut Vec<Span<'a>>) {
    paragraph.push(Line::from_iter(line.clone()));
    line.clear();
}

impl Widget for &MarkdownView {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut paragraph = vec![];
        let mut line = vec![];

        let mut heading = false;
        let mut list_level = 0;

        let mut last_tag: Option<TagEnd> = None;

        // TODO: This implementation lacks several features and could be improved in a
        // multitude of ways.

        let parser = Parser::new(&self.content);
        for event in parser {
            match event {
                Event::Text(text) => {
                    let mut style = Style::new();
                    if heading {
                        style = style.bold().light_blue();
                    }
                    line.push(Span::styled(text, style));
                }
                Event::Start(tag) => match tag {
                    Tag::Heading {
                        level: _,
                        id: _,
                        classes: _,
                        attrs: _,
                    } => {
                        if let Some(_) = last_tag {
                            flush_line(&mut paragraph, &mut line);
                        }
                        heading = true;
                    }
                    Tag::Paragraph => {
                        // Paragraphs should have spacing between.
                        if let Some(TagEnd::Paragraph) = last_tag {
                            flush_line(&mut paragraph, &mut line);
                        }
                    }
                    Tag::List(_) => {
                        // If this list is indented, this new list will be embedded into an item tag.
                        // We need to flush the old item tag if this is the case.
                        if !line.is_empty() {
                            flush_line(&mut paragraph, &mut line);
                        }
                        list_level += 1;
                    }
                    Tag::Item => {
                        let text_element = Span::raw("  ".repeat(list_level - 1) + "- ");
                        line.push(text_element);
                    }
                    _ => {}
                },
                Event::End(tag) => {
                    match tag {
                        TagEnd::Heading(_) => {
                            flush_line(&mut paragraph, &mut line);
                            flush_line(&mut paragraph, &mut line);
                            heading = false;
                        }
                        TagEnd::Paragraph => {
                            flush_line(&mut paragraph, &mut line);
                        }
                        TagEnd::List(_) => list_level -= 1,
                        TagEnd::Item => {
                            // It is possible that this particular item has already been flushed. See
                            // how List tags are entered.
                            if !line.is_empty() {
                                flush_line(&mut paragraph, &mut line);
                            }
                        }
                        _ => {}
                    }
                    last_tag = Some(tag);
                }
                _ => {}
            }
        }
        flush_line(&mut paragraph, &mut line);

        Paragraph::new(paragraph)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_lines.try_into().unwrap(), 0))
            .render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn test_render_view() {
        let view = MarkdownView::new(
            "
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

This should not be visible.
        ",
        );
        let mut terminal = Terminal::new(TestBackend::new(80, 21)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&view, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());
    }
}
