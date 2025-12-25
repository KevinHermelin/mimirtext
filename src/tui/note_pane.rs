use ratatui::{
    buffer::Buffer,
    layout::{Offset, Position, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols::border,
    text::Line,
    widgets::{Block, Paragraph, Widget, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::{
    model::note_pane::{Document, NotePaneModel, NotePaneState, ViewMode},
    tui::{WidgetWithCursor, markdown_view::MarkdownView, utils::NonIdealState},
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

        if let Some(progress) = self.graph_build_progress.clone() {
            let progress_text = format!(" building graph ({:.0}%) ", progress.percentage() * 100.0);
            block = block.title_top(Line::from(progress_text).dim().right_aligned());
        }

        if let NotePaneState::With(context) = &self.state {
            let mut title_text = context.note.title.clone();
            let mut title_style = Style::new().add_modifier(Modifier::BOLD);
            if context.modified() {
                title_text.push('*');
                title_style = title_style.add_modifier(Modifier::ITALIC);
            }

            block = block
                .title_bottom(Line::styled(format!(" {} ", title_text), title_style).left_aligned())
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
    pub fn clean_text(&self, text: &str) -> String {
        // Tabs seem to render weirdly in the Ratatui.
        let tab_columns = self.editor_config().tab_columns;
        assert!(tab_columns >= 1);

        text.replace('\t', &(String::from("⇥") + &" ".repeat(tab_columns - 1)))
    }
}

impl WidgetWithCursor for NotePaneModel {
    fn render_with_cursor(&self, area: Rect, buf: &mut Buffer) -> Option<Position> {
        let area = self.render_block(area, buf);
        match &self.state {
            NotePaneState::Empty => {
                NonIdealState::new("Mimir", "Press <CTRL+P> to open a note").render(area, buf)
            }
            NotePaneState::Loading(_) => NonIdealState::new("Loading note", "").render(area, buf),
            NotePaneState::With(context) => {
                let document = context.document();

                if let Some(editor) = &context.editor {
                    let max_cols = area.width;
                    let max_rows = area.height;
                    let page_cols = editor.cursor_column() as u16 / max_cols;
                    let page_rows = editor.cursor_row() as u16 / max_rows;
                    let scroll_col = page_cols * max_cols;
                    let scroll_row = page_rows * max_rows;

                    let document = editor.text();
                    let document = self.clean_text(&document);

                    Paragraph::new(document)
                        .scroll((scroll_row, scroll_col))
                        .render(area, buf);
                    let cursor = Position {
                        x: area.x + editor.cursor_column() as u16 % max_cols,
                        y: area.y + editor.cursor_row() as u16 % max_rows,
                    };

                    if let Some(completions) = editor.completions() {
                        let mut completion_area = Rect {
                            x: cursor.x,
                            y: cursor.y + 1,
                            width: 40,
                            height: 5,
                        };

                        let mut lines = vec![];
                        for completion in completions.items().iter().take(5) {
                            let text = completion.display_text.clone();

                            // To get the entire block filled.
                            let fill_width =
                                (completion_area.width as usize).saturating_sub(text.width());

                            let text = text.clone() + &" ".repeat(fill_width);
                            let mut line = Line::from(text).bg(Color::DarkGray);

                            if completions.selected() == completion {
                                line = line.reversed();
                            }

                            lines.push(line);
                        }

                        // Move area so it fits in the pane.
                        let overflow_x = completion_area.right().saturating_sub(area.right());
                        let overflow_y = completion_area.bottom().saturating_sub(area.bottom());
                        if overflow_x > 0 {
                            completion_area = completion_area.offset(Offset {
                                x: -(overflow_x as i32),
                                y: 0,
                            });
                        }
                        if overflow_y > 0 {
                            completion_area = completion_area.offset(Offset {
                                x: 0,
                                y: -(completion_area.height as i32) - 1,
                            });
                        }
                        Paragraph::new(lines).render(completion_area, buf);
                    }

                    return Some(cursor);
                }

                if context.note.body.is_empty() {
                    NonIdealState::new("This note is empty", "Press <C> to open in editor")
                        .render(area, buf);
                    return None;
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
                        Paragraph::new(self.clean_text(&source))
                            .scroll((context.scroll_lines as u16, 0))
                            .wrap(Wrap { trim: false })
                            .render(area, buf);
                    }
                }
            }
        };
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{Update, note_pane::NotePaneMessage},
        repository::{MockRepository, Repository},
        text_input::{Completion, InputOperation},
        tui::GraphBuildProgress,
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
        let model = NotePaneModel::default();

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| {
                model.render_with_cursor(frame.area(), frame.buffer_mut());
            })
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_graph_building() {
        let model = NotePaneModel::default();
        let (model, _) = model.update(NotePaneMessage::GraphUpdate(GraphBuildProgress(51, 200)));

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| {
                model.render_with_cursor(frame.area(), frame.buffer_mut());
            })
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
            .draw(|frame| {
                model.render_with_cursor(frame.area(), frame.buffer_mut());
            })
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
            .draw(|frame| {
                model.render_with_cursor(frame.area(), frame.buffer_mut());
            })
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_editing_note_modified() {
        let note = MockRepository::new().insert_note("Note name.md", "We are editing this note");

        let (model, _) = NotePaneModel::default().update(NotePaneMessage::PushNote(note));
        let (model, _) = model.update(NotePaneMessage::StartEdit);
        let (model, _) = model.update(NotePaneMessage::Input(InputOperation::Down));
        let (model, _) = model.update(NotePaneMessage::Input(InputOperation::Insert(
            String::from(" and adding some text. \nNote name should have an \"*\" appended to it."),
        )));

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| {
                model.render_with_cursor(frame.area(), frame.buffer_mut());
            })
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_editing_note_scroll() {
        let note = MockRepository::new().insert_note("Note name.md", "");

        let (model, _) = NotePaneModel::default().update(NotePaneMessage::PushNote(note));
        let (model, _) = model.update(NotePaneMessage::StartEdit);
        let (model, _) = model.update(NotePaneMessage::Input(InputOperation::Insert(
            String::from("This\nnote\nspans\nmore\nlines\nthan\ncan\nbe\nshown. And this line is also longer than the width of the screen."),
        )));

        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| {
                model.render_with_cursor(frame.area(), frame.buffer_mut());
            })
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_editing_note_completions() {
        let note = MockRepository::new().insert_note("Note name.md", "");

        let (model, _) = NotePaneModel::default().update(NotePaneMessage::PushNote(note));
        let (model, _) = model.update(NotePaneMessage::StartEdit);
        let (model, _) = model.update(NotePaneMessage::Input(InputOperation::Insert(
            String::from("[["),
        )));
        let (model, _) = model.update(NotePaneMessage::UpdateCompletion(vec![
            Completion::note_link(0..2, "note"),
        ]));

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| {
                model.render_with_cursor(frame.area(), frame.buffer_mut());
            })
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
            .draw(|frame| {
                model.render_with_cursor(frame.area(), frame.buffer_mut());
            })
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
            .draw(|frame| {
                model.render_with_cursor(frame.area(), frame.buffer_mut());
            })
            .unwrap();
        assert_snapshot!(terminal.backend());

        Ok(())
    }

    #[test]
    fn test_clean() {
        assert_eq!(NotePaneModel::default().editor_config().tab_columns, 2);
        assert_eq!(
            NotePaneModel::default().clean_text("Contains a \t tab."),
            "Contains a ⇥  tab."
        );
    }
}
