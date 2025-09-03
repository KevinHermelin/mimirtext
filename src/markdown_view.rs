use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::Text,
    widgets::{Paragraph, Widget},
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

impl Widget for MarkdownView {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Tabs render very poorly in paragraph. Better to use spaces.
        let text = self.content.replace("\t", "  ");

        let buffer_text = Text::from(text);
        Paragraph::new(buffer_text)
            .scroll((self.scroll_lines.try_into().unwrap(), 0))
            .render(area, buf);
    }
}
