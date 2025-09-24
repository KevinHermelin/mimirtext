use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::Stylize,
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

use crate::{model::SearchWindowModel, repository::NoteKey};

impl Widget for &SearchWindowModel {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title("Search")
            .border_set(border::ROUNDED);

        let inner = block.inner(area);
        block.render(area, buf);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(2), Constraint::Min(5)])
            .split(inner);

        let search_line = if self.input.text().is_empty() {
            Line::from("Type to search...").dim()
        } else {
            Line::from(self.input.text())
        };
        search_line.render(layout[0], buf);

        let mut lines = vec![];
        for (index, result) in self.results.iter().cloned().enumerate() {
            let NoteKey(_, note_id) = result.key;
            let is_selected = index == self.selection_index as usize;

            let mut line = Line::from(note_id);

            if is_selected {
                line.spans.insert(0, Span::from("> "));
                line = line.light_blue();
            } else {
                line.spans.insert(0, Span::from("  "));
            }

            lines.push(line);
        }

        Paragraph::new(lines).render(layout[1], buf);
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend};

    use crate::{
        model::SearchWindowModel,
        repository::{MockRepository, Repository, SearchResult},
        text_input::TextInput,
    };

    #[test]
    fn test_default() {
        let model = SearchWindowModel::default();

        let mut terminal = Terminal::new(TestBackend::new(20, 10)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&model, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_with_search() {
        let repo = MockRepository::new();

        let model = SearchWindowModel {
            input: TextInput::from("Search query"),
            results: vec![
                SearchResult::new(repo.note_key("Result A"), 1.0),
                SearchResult::new(repo.note_key("Result B"), 1.0),
                SearchResult::new(repo.note_key("Result C"), 1.0),
                SearchResult::new(repo.note_key("Result D"), 1.0),
            ],
            selection_index: 2,
        };

        let mut terminal = Terminal::new(TestBackend::new(20, 10)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&model, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());
    }
}
