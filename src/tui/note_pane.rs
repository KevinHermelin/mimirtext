use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::Line,
    widgets::{Block, Widget},
};

use crate::{
    model::{NotePaneModel, NotePaneState},
    tui::{markdown_view::MarkdownView, utils::NonIdealState},
};

impl NotePaneModel {
    fn render_block(&self, title: &str, area: Rect, buf: &mut Buffer) -> Rect {
        let title = Line::from(title.to_string().bold());
        let instructions = Line::from(vec![" Exit ".into(), "<Q> ".blue().bold()]);
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.right_aligned())
            .border_set(border::THICK);
        let inner_area = block.inner(area);
        block.render(area, buf);

        inner_area
    }
}

impl Widget for &NotePaneModel {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match &self.state {
            NotePaneState::NoNote => {
                let area = self.render_block("", area, buf);

                NonIdealState::new("Mimir", "No opened notes").render(area, buf)
            }
            NotePaneState::LoadingNote(_) => {
                let area = self.render_block("", area, buf);

                NonIdealState::new("Loading note", "").render(area, buf)
            }
            NotePaneState::WithNote(context) => {
                let area = self.render_block(&context.note.title, area, buf);

                let mut document = context.document();
                // TODO: There are several problems here. For one, this is untested.
                // It is also weird that we set the selected_link during render
                // and from the names alone we are not giving any clues that context.selected_link
                // does not retrieve that information from the document.
                document.selected_link = context.selected_link();

                MarkdownView::new(document)
                    .scroll(context.scroll_lines as i16)
                    .render(area, buf);
                if context.note.body.is_empty() {
                    NonIdealState::new("This note is empty", "Press <C> to open in editor")
                        .render(area, buf);
                }
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{NoteContext, NotePaneMessage, Update},
        repository::{MockRepository, Repository},
    };
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend};
    use std::io::Result;

    #[test]
    fn test_no_note() {
        let mut model = NotePaneModel::default();
        model.state = NotePaneState::NoNote;

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&model, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_with_note() {
        let note = MockRepository::new().insert_note(
            "Note name.md",
            "this should not be visible.\n\nthis is a test file\n\nwith multiple\n\nparagraphs",
        );

        let mut model = NotePaneModel::default();
        model.state = NotePaneState::WithNote(NoteContext {
            note,
            scroll_lines: 1,
            link_selection_index: 0,
        });

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&model, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_with_new_note() -> Result<()> {
        let note = MockRepository::new().note("new note.md")?;

        let model = NotePaneModel::default();
        let (model, _) = model.update(NotePaneMessage::PushNote(note));

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&model, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());

        Ok(())
    }
}
