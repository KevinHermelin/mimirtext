use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::Stylize,
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

use crate::{model::search_window::SearchWindowModel, repository::NoteKey, tui::WidgetWithCursor};

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

        let create_new_label = Line::from(" <CTRL+J> ").dim().right_aligned();

        let mut search_line = Line::from(format!("{}:", self.repo_id)).bold().light_blue();
        let mut cursor_x = search_line.width();

        let search_bar = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Min(cursor_x.try_into().unwrap()),
                Constraint::Length(create_new_label.width().try_into().unwrap()),
            ])
            .split(layout[0]);

        if self.input.text().is_empty() {
            search_line.push_span("note name".reset().dim());
        } else {
            search_line.push_span(self.input.text().reset());
            cursor_x += self.input.cursor_column();

            create_new_label.render(search_bar[1], buf);
        };

        search_line.render(search_bar[0], buf);
        let cursor = Some(Position::new(
            layout[0].x + cursor_x as u16,
            search_bar[0].y,
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
        graph::SearchResult,
        model::{
            Update,
            search_window::{SearchWindowMessage, SearchWindowModel},
        },
        repository::{MockRepository, Repository},
        text_input::TextInput,
        tui::WidgetWithCursor,
    };

    #[test]
    fn test_default() {
        let model = SearchWindowModel::new("repo");

        let mut terminal = Terminal::new(TestBackend::new(65, 10)).unwrap();
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
            repo_id: String::from("repo"),
        };

        let mut terminal = Terminal::new(TestBackend::new(65, 10)).unwrap();
        terminal
            .draw(|frame| {
                model.render_with_cursor(frame.area(), frame.buffer_mut());
            })
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_cursor() {
        let model = SearchWindowModel::new("repo");
        let mut terminal = Terminal::new(TestBackend::new(65, 10)).unwrap();

        assert_eq!(
            model.render_with_cursor(terminal.get_frame().area(), terminal.current_buffer_mut()),
            Some(Position::new(1 + 4 + 1, 1))
        );

        let (model, _) = model.update(SearchWindowMessage::Input(
            crate::text_input::InputOperation::Insert(String::from("Test")),
        ));

        assert_eq!(
            model.render_with_cursor(terminal.get_frame().area(), terminal.current_buffer_mut()),
            Some(Position::new(1 + 4 + 1 + 4, 1))
        );

        let model = SearchWindowModel::new("repo");
        let (model, _) = model.update(SearchWindowMessage::Input(
            crate::text_input::InputOperation::Insert(String::from("åäö")),
        ));

        assert_eq!(
            model.render_with_cursor(terminal.get_frame().area(), terminal.current_buffer_mut()),
            Some(Position::new(1 + 4 + 1 + 3, 1))
        );
    }
}
