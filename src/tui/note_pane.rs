use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Stylize},
    symbols::border,
    text::Line,
    widgets::{Block, Paragraph, Widget, Wrap},
};

use crate::{
    model::{Document, NotePaneModel, NotePaneState},
    tui::{markdown_view::MarkdownView, utils::NonIdealState},
};

#[derive(Debug, PartialEq)]
enum Mode {
    None,
    Source,
    Browsing,
}

impl Mode {
    fn name(&self) -> String {
        match self {
            Mode::None => String::from("-"),
            Mode::Source => String::from("source"),
            Mode::Browsing => String::from("browse"),
        }
    }

    fn color(&self) -> Color {
        match self {
            Mode::None => Color::Gray,
            Mode::Source => Color::Gray,
            Mode::Browsing => Color::Cyan,
        }
    }
}

impl NotePaneModel {
    fn render_block(&self, title: Option<&str>, mode: Mode, area: Rect, buf: &mut Buffer) -> Rect {
        let mut block = Block::bordered().border_set(border::THICK);

        if let Some(title) = title {
            block = block.title_bottom(Line::from(format!(" {} ", title)).bold().left_aligned())
        }

        if mode != Mode::None {
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
        match &self.state {
            NotePaneState::NoNote => {
                let area = self.render_block(None, Mode::None, area, buf);

                NonIdealState::new("Mimir", "Press <CTRL+P> to open a note").render(area, buf)
            }
            NotePaneState::LoadingNote(_) => {
                let area = self.render_block(None, Mode::None, area, buf);

                NonIdealState::new("Loading note", "").render(area, buf)
            }
            NotePaneState::WithNote(context) => {
                let document = context.document();
                if context.note.body.is_empty() {
                    let area =
                        self.render_block(Some(&context.note.title), Mode::Browsing, area, buf);
                    NonIdealState::new("This note is empty", "Press <C> to open in editor")
                        .render(area, buf);
                    return;
                }

                match document {
                    Document::Markdown(mut document) => {
                        let area =
                            self.render_block(Some(&context.note.title), Mode::Browsing, area, buf);

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
                        let area =
                            self.render_block(Some(&context.note.title), Mode::Source, area, buf);

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
        model::{NoteContext, NotePaneMessage, Update},
        repository::{MockRepository, Repository},
    };
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend};
    use std::io::Result;

    #[test]
    fn test_mode() {
        assert_eq!(Mode::None.name(), String::from("-"));
        assert_eq!(Mode::Browsing.name(), String::from("browse"));
        assert_eq!(Mode::Source.name(), String::from("source"));
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
