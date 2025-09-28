use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Stylize},
    symbols::border,
    text::Line,
    widgets::{Block, Paragraph, Widget, Wrap},
};

use crate::{
    model::{Document, NotePaneModel, NotePaneState, ViewMode},
    tui::{markdown_view::MarkdownView, utils::NonIdealState},
};

impl ViewMode {
    fn name(&self) -> String {
        match self {
            ViewMode::None => String::from("-"),
            ViewMode::Source => String::from("source"),
            ViewMode::Browsing => String::from("browse"),
            ViewMode::Edit => String::from("edit"),
        }
    }

    fn color(&self) -> Color {
        match self {
            ViewMode::None => Color::Gray,
            ViewMode::Source => Color::Gray,
            ViewMode::Browsing => Color::Cyan,
            ViewMode::Edit => Color::Red,
        }
    }
}

impl NotePaneModel {
    fn render_block(&self, area: Rect, buf: &mut Buffer) -> Rect {
        let mut block = Block::bordered().border_set(border::THICK);

        if let NotePaneState::WithNote(context) = &self.state {
            block = block.title_bottom(
                Line::from(format!(" {} ", context.note.title))
                    .bold()
                    .left_aligned(),
            )
        }

        let mode = self.view_mode();
        if mode != ViewMode::None {
            block = block.title_bottom(
                Line::from(format!(" {} ", mode.name().to_uppercase()))
                    .fg(mode.color())
                    .right_aligned(),
            )
        }

        let inner_area = block.inner(area);
        block.render(area, buf);

        inner_area
    }
}

impl Widget for &NotePaneModel {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = self.render_block(area, buf);
        match &self.state {
            NotePaneState::NoNote => {
                NonIdealState::new("Mimir", "Press <CTRL+P> to open a note").render(area, buf)
            }
            NotePaneState::LoadingNote(_) => {
                NonIdealState::new("Loading note", "").render(area, buf)
            }
            NotePaneState::WithNote(context) => {
                let document = context.document();
                if context.note.body.is_empty() {
                    NonIdealState::new("This note is empty", "Press <C> to open in editor")
                        .render(area, buf);
                    return;
                }

                match document {
                    Document::Markdown(mut document) => {
                        // TODO: There are several problems here. For one, this is untested.
                        // It is also weird that we set the selected_link during render
                        // and from the names alone we are not giving any clues that context.selected_link
                        // does not retrieve that information from the document.
                        document.selected_link = context.selected_link();

                        MarkdownView::new(document)
                            .scroll(context.scroll_lines as i16)
                            .render(area, buf);
                    }
                    Document::Source(source) => {
                        Paragraph::new(source)
                            .scroll((context.scroll_lines as u16, 0))
                            .wrap(Wrap { trim: false })
                            .render(area, buf);
                    }
                }
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{NotePaneMessage, Update},
        repository::{MockRepository, Repository},
    };
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend};
    use std::io::Result;

    #[test]
    fn test_mode() {
        assert_eq!(ViewMode::None.name(), String::from("-"));
        assert_eq!(ViewMode::Browsing.name(), String::from("browse"));
        assert_eq!(ViewMode::Source.name(), String::from("source"));
    }

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

        let (model, _) = NotePaneModel::default().update(NotePaneMessage::PushNote(note));
        let (model, _) = model.update(NotePaneMessage::ScrollDown);

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&model, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_editing_note() {
        let note = MockRepository::new().insert_note("Note name.md", "We are editing this note");

        let (model, _) = NotePaneModel::default().update(NotePaneMessage::PushNote(note));
        let (model, _) = model.update(NotePaneMessage::StartEdit);

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&model, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_with_raw_note() {
        let note = MockRepository::new().insert_note(
            "Note without extension",
            "# This is not a markdown document and should not be rendered as such. \n- This should not be rendered as a list\n- And [[this is|not a link]]",
        );

        let model = NotePaneModel::default();
        let (model, _) = model.update(NotePaneMessage::PushNote(note));

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
