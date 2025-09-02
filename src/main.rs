mod file_buffer;

use clap::{Parser, command};
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
};
use std::path::PathBuf;

use crate::file_buffer::FileBuffer;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    file_path: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let file_path = cli.file_path;
    let buffer = FileBuffer::from_file(&file_path).expect("should be able to read file");

    let mut app = App {
        buffer,
        scroll_offset_line: 0,
        should_exit: false,
    };

    color_eyre::install()?;
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

pub struct App {
    buffer: FileBuffer,
    scroll_offset_line: i16,
    should_exit: bool,
}

enum Action {
    ScrollUp,
    ScrollDown,
    Exit,
    None,
}

impl App {
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
            Action::None => {}
        }
    }
    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }
    fn move_scroll(&mut self, lines: i16) {
        let number_of_lines: i16 = self.buffer.content.lines().count().try_into().unwrap();

        self.scroll_offset_line = (self.scroll_offset_line + lines).clamp(0, number_of_lines - 1);
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

        let buffer_text = Text::from(self.buffer.content.clone());

        Paragraph::new(buffer_text)
            .block(block)
            .scroll((self.scroll_offset_line.try_into().unwrap(), 0))
            .render(area, buf);
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
                    "this should not be visible.\nthis is a test file\nspanning multiple\nlines",
                ),
            },
            scroll_offset_line: 1,
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
            scroll_offset_line: 0,
            should_exit: false,
        };

        // Can not scroll up when on first line.
        app.handle_action(Action::ScrollUp);
        assert_eq!(app.scroll_offset_line, 0);
        app.handle_action(Action::ScrollDown);
        assert_eq!(app.scroll_offset_line, 1);
        app.handle_action(Action::ScrollDown);
        assert_eq!(app.scroll_offset_line, 2);
        // Scrolling beyond last line should not be possible.
        app.handle_action(Action::ScrollDown);
        assert_eq!(app.scroll_offset_line, 2);
        app.handle_action(Action::ScrollUp);
        assert_eq!(app.scroll_offset_line, 1);
    }

    #[test]
    fn test_handle_quit_action() {
        let mut app = App {
            buffer: FileBuffer {
                file_path: PathBuf::from("example.txt"),
                content: String::from("this is a test file\nspanning multiple\nlines"),
            },
            scroll_offset_line: 0,
            should_exit: false,
        };

        app.handle_action(Action::Exit);
        assert_eq!(app.should_exit, true);
    }
}
