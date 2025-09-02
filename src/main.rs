mod file_buffer;

use clap::{Parser, command};
use color_eyre::Result;
use crossterm::event::{self, Event};
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

    let mut app = App { buffer };

    color_eyre::install()?;
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

pub struct App {
    buffer: FileBuffer,
}

impl App {
    fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;
            if matches!(event::read()?, Event::Key(_)) {
                break Ok(());
            }
        }
    }
    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from(self.buffer.file_path.display().to_string().bold());
        let instructions = Line::from(vec![" Quit ".into(), "<Any Key> ".blue().bold()]);
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK);

        let buffer_text = Text::from(self.buffer.content.clone());

        Paragraph::new(buffer_text).block(block).render(area, buf);
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
                content: String::from("this is a test file"),
            },
        };
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&app, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());
    }
}
