mod file_buffer;
mod markdown_view;

use clap::{Parser, command};
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::Line,
    widgets::{Block, Clear, Widget},
};
use std::path::PathBuf;

use crate::{
    file_buffer::FileBuffer,
    markdown_view::{MarkdownDocument, MarkdownView},
};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    file_path: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let file_path = cli.file_path;
    let buffer = FileBuffer::from_file(&file_path).expect("should be able to read file");

    let mut app = App::new(buffer);

    color_eyre::install()?;
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

pub struct App {
    buffer: FileBuffer,
    scroll_lines: i16,
    link_selection_index: isize,
    should_exit: bool,
}

enum Action {
    ScrollUp,
    ScrollDown,
    Exit,
    None,
    NextLink,
    PreviousLink,
}

impl App {
    fn new(buffer: FileBuffer) -> Self {
        return App {
            buffer,
            scroll_lines: 0,
            link_selection_index: 0,
            should_exit: false,
        };
    }
    fn get_document(&self) -> MarkdownDocument {
        MarkdownDocument::new(&self.buffer.content)
    }
    fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_event()?;
        }
        Ok(())
    }
    fn handle_event(&mut self) -> Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        let action = match key_event.code {
            KeyCode::Down => Action::ScrollDown,
            KeyCode::Up => Action::ScrollUp,
            KeyCode::Right => Action::NextLink,
            KeyCode::Left => Action::PreviousLink,
            KeyCode::Char('q') => Action::Exit,
            _ => Action::None,
        };
        self.handle_action(action);
    }
    fn handle_action(&mut self, action: Action) {
        match action {
            Action::ScrollDown => self.move_scroll(1),
            Action::ScrollUp => self.move_scroll(-1),
            Action::Exit => self.should_exit = true,
            Action::NextLink => self.move_link_selection(1),
            Action::PreviousLink => self.move_link_selection(-1),
            Action::None => {}
        }
    }
    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(Clear, frame.area());
        frame.render_widget(self, frame.area());
    }
    fn move_scroll(&mut self, lines: i16) {
        let number_of_lines: i16 = self.buffer.content.lines().count().try_into().unwrap();

        self.scroll_lines = (self.scroll_lines + lines).clamp(0, number_of_lines - 1);
    }
    fn move_link_selection(&mut self, offset: isize) {
        let link_count = self.get_document().get_links().len().try_into().unwrap();

        if link_count == 0 {
            self.link_selection_index = 0;
            return;
        }

        self.link_selection_index = (self.link_selection_index + offset).rem_euclid(link_count);
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from(self.buffer.file_path.display().to_string().bold());
        let instructions = Line::from(vec![
            " Scroll Up/Down ".into(),
            "<Up>/<Down> ".blue().bold(),
            " Exit ".into(),
            "<Q> ".blue().bold(),
        ]);
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK);
        let inner_area = block.inner(area);

        block.render(area, buf);

        let mut document = self.get_document();
        document.selected_link = document
            .get_links()
            .get::<usize>(self.link_selection_index.try_into().unwrap())
            .cloned();

        MarkdownView::new(document)
            .scroll(self.scroll_lines.try_into().unwrap())
            .render(inner_area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn test_render_buffer() {
        let app = App {
            buffer: FileBuffer {
                file_path: PathBuf::from("example.txt"),
                content: String::from(
                    "this should not be visible.\n\nthis is a test file\n\nwith multiple\n\nparagraphs",
                ),
            },
            link_selection_index: 0,
            scroll_lines: 1,
            should_exit: false,
        };
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&app, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_handle_scroll_action() {
        let mut app = App {
            buffer: FileBuffer {
                file_path: PathBuf::from("example.txt"),
                content: String::from("this is a test file\nspanning multiple\nlines"),
            },
            link_selection_index: 0,
            scroll_lines: 0,
            should_exit: false,
        };

        // Can not scroll up when on first line.
        app.handle_action(Action::ScrollUp);
        assert_eq!(app.scroll_lines, 0);
        app.handle_action(Action::ScrollDown);
        assert_eq!(app.scroll_lines, 1);
        app.handle_action(Action::ScrollDown);
        assert_eq!(app.scroll_lines, 2);
        // Scrolling beyond last line should not be possible.
        app.handle_action(Action::ScrollDown);
        assert_eq!(app.scroll_lines, 2);
        app.handle_action(Action::ScrollUp);
        assert_eq!(app.scroll_lines, 1);
    }

    #[test]
    fn test_handle_quit_action() {
        let mut app = App {
            buffer: FileBuffer {
                file_path: PathBuf::from("example.txt"),
                content: String::from("this is a test file\nspanning multiple\nlines"),
            },
            link_selection_index: 0,
            scroll_lines: 0,
            should_exit: false,
        };

        app.handle_action(Action::Exit);
        assert_eq!(app.should_exit, true);
    }

    #[test]
    fn test_handle_link_selection() {
        let mut app = App::new(FileBuffer {
            file_path: PathBuf::from("example.txt"),
            content: String::from("[[This]] file [[Have|has]] multiple [links](url.com)"),
        });

        assert_eq!(app.link_selection_index, 0);

        app.handle_action(Action::NextLink);
        assert_eq!(app.link_selection_index, 1);

        app.handle_action(Action::NextLink);
        assert_eq!(app.link_selection_index, 2);

        app.handle_action(Action::NextLink);
        assert_eq!(app.link_selection_index, 0);

        app.handle_action(Action::PreviousLink);
        assert_eq!(app.link_selection_index, 2);

        app.handle_action(Action::PreviousLink);
        assert_eq!(app.link_selection_index, 1);

        let mut app = App::new(FileBuffer {
            file_path: PathBuf::from("example.txt"),
            content: String::from("This file has no link"),
        });

        assert_eq!(app.link_selection_index, 0);

        app.handle_action(Action::NextLink);
        assert_eq!(app.link_selection_index, 0);
    }
}
