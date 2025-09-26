use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::Stylize,
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

use crate::{model::SearchWindowModel, repository::NoteKey, tui::WidgetWithCursor};

impl WidgetWithCursor for SearchWindowModel {
    fn render_with_cursor(&self, area: Rect, buf: &mut Buffer) -> Option<Position> {
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
        let cursor = Some(Position::new(
            layout[0].x + self.input.cursor_column() as u16,
            layout[0].y,
        ));

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
        cursor
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend, layout::Position};

    use crate::{
        model::{SearchWindowMessage, SearchWindowModel, Update},
        repository::{MockRepository, Repository, SearchResult},
        text_input::TextInput,
        tui::WidgetWithCursor,
    };

    #[test]
    fn test_default() {
        let model = SearchWindowModel::default();

        let mut terminal = Terminal::new(TestBackend::new(20, 10)).unwrap();
        terminal
            .draw(|frame| {
                model.render_with_cursor(frame.area(), frame.buffer_mut());
            })
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
            .draw(|frame| {
                model.render_with_cursor(frame.area(), frame.buffer_mut());
            })
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_cursor() {
        let model = SearchWindowModel::default();
        let mut terminal = Terminal::new(TestBackend::new(20, 10)).unwrap();

        assert_eq!(
            model.render_with_cursor(terminal.get_frame().area(), terminal.current_buffer_mut()),
            Some(Position::new(1, 1))
        );

        let (model, _) = model.update(SearchWindowMessage::Input(
            crate::text_input::InputOperation::Insert(String::from("Test")),
        ));

        assert_eq!(
            model.render_with_cursor(terminal.get_frame().area(), terminal.current_buffer_mut()),
            Some(Position::new(5, 1))
        );

        let model = SearchWindowModel::default();
        let (model, _) = model.update(SearchWindowMessage::Input(
            crate::text_input::InputOperation::Insert(String::from("åäö")),
        ));

        assert_eq!(
            model.render_with_cursor(terminal.get_frame().area(), terminal.current_buffer_mut()),
            Some(Position::new(4, 1))
        );
    }
}
