mod input;
pub mod markdown_view;
mod note_pane;
mod search_window;
pub mod utils;

use clap::{Parser, command};
use color_eyre::Result;
use ratatui::{
    Frame, Terminal,
    buffer::Buffer,
    crossterm::{
        ExecutableCommand,
        event::{self, Event, KeyEvent, KeyEventKind},
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    layout::{Constraint, Position, Rect},
    prelude::Backend,
    style::Stylize,
    symbols::border,
    text::Line,
    widgets::{Block, Clear, Paragraph, Widget},
};
use std::{
    io::{self, stdout},
    path::{Path, PathBuf},
};

use crate::{
    markdown::LinkTarget,
    model::{
        Command, Message, Model, RunningState, Update, note_pane::NotePaneMessage,
        search_window::SearchWindowMessage,
    },
    repository::{FolderRepository, NoteKey, NoteSnapshot, Repository},
    text_input::Completion,
    tui::{input::KeyHandler, utils::center},
};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(default_value = ".")]
    path: PathBuf,
}

pub fn main() -> Result<()> {
    let cli = Cli::parse();

    let file_path = cli.path;
    let mut app = App::from_path(&file_path)?;

    color_eyre::install()?;
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

pub struct App {
    model: Model,
    repository: Box<dyn Repository>,
    error_message: Option<String>,
}

pub trait WidgetWithCursor {
    fn render_with_cursor(&self, area: Rect, buf: &mut Buffer) -> Option<Position>;
}

impl App {
    fn new(repository: Box<dyn Repository>, note: Option<NoteSnapshot>) -> Self {
        let mut model = Model::default();

        if let Some(note) = note {
            let (new_model, command) =
                model.update(Message::NotePane(NotePaneMessage::PushNote(note)));
            model = new_model;
            assert_eq!(command, Command::None);
        }

        App {
            model,
            repository,
            error_message: None,
        }
    }
    fn from_path(path: &Path) -> io::Result<Self> {
        let (repo, note) = FolderRepository::open_path(path)?;

        Ok(App::new(Box::new(repo), note))
    }
    fn run(&mut self, terminal: &mut Terminal<impl Backend>) -> Result<()> {
        let mut message = Message::None;
        while self.model.running_state != RunningState::Done {
            terminal.draw(|frame| self.draw(frame))?;

            if let Message::None = message {
                message = self.handle_event()?;
            }

            let (model, command) = self.model.update(message);
            message = Message::None;
            self.model = model;

            match command {
                Command::EditExternally(note) => {
                    stdout().execute(LeaveAlternateScreen)?;
                    disable_raw_mode()?;

                    let NoteKey(repo_id, note_id) = note;
                    assert_eq!(repo_id, self.repository.id());

                    self.repository.edit_externally(&note_id)?;
                    let note = self.repository.note(&note_id)?;
                    message = Message::NotePane(NotePaneMessage::UpdateNote(note));

                    stdout().execute(EnterAlternateScreen)?;
                    enable_raw_mode()?;
                    terminal.clear()?;
                }
                Command::FollowLink(link) => match link {
                    LinkTarget::Note(target) => {
                        let id = self.repository.resolve_reference(&target);
                        let note = self.repository.note(&id)?;
                        message = Message::NotePane(NotePaneMessage::PushNote(note));
                    }
                    LinkTarget::External(target) => {
                        open::that(&target)?;
                    }
                },
                Command::ServeNote(note) => {
                    let NoteKey(repo_id, id) = note;
                    assert_eq!(repo_id, self.repository.id());
                    let note = self.repository.note(&id)?;
                    message = Message::NotePane(NotePaneMessage::UpdateNote(note));
                }
                Command::SearchQuery(query) => {
                    let results = self.repository.search(&query)?;
                    message = Message::SearchWindow(SearchWindowMessage::UpdateResults(results));
                }
                Command::OpenNote(key) => {
                    let NoteKey(_, note_id) = key;
                    let note = self.repository.note(&note_id)?;
                    message = Message::NotePane(NotePaneMessage::PushNote(note));
                }
                Command::CommitNote(note) => {
                    let NoteKey(repo_id, note_id) = &note.key.clone();
                    assert_eq!(repo_id, self.repository.id());
                    self.repository.commit_note(note)?;

                    let note = self.repository.note(note_id)?;
                    message = Message::NotePane(NotePaneMessage::UpdateNote(note));
                }
                Command::RequestLinkCompletion(range, text) => {
                    let results = self.repository.search(&text)?;
                    let completions = results
                        .iter()
                        .map(|result| result.key.1.clone().split(".md").next().unwrap().to_owned())
                        .map(|reference| Completion::note_link(range.clone(), &reference))
                        .collect();

                    message = Message::NotePane(NotePaneMessage::UpdateCompletion(completions));
                }
                Command::None => {}
            }
        }
        Ok(())
    }
    fn handle_event(&mut self) -> Result<Message> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                Ok(self.handle_key_event(key_event))
            }
            _ => Ok(Message::None),
        }
    }
    fn handle_key_event(&mut self, key_event: KeyEvent) -> Message {
        if self.error_message.is_some() {
            self.error_message = None;
            return Message::None;
        }

        self.model.handle_key_event(key_event)
    }
    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(Clear, frame.area());
        let cursor = self.render_with_cursor(frame.area(), frame.buffer_mut());

        if let Some(position) = cursor {
            frame.set_cursor_position(position);
        }
    }
}

impl WidgetWithCursor for App {
    fn render_with_cursor(&self, area: Rect, buf: &mut Buffer) -> Option<Position> {
        let cursor = self.model.render_with_cursor(area, buf);

        if let Some(error) = self.error_message.to_owned() {
            let block = Block::bordered().title("Error").border_set(border::ROUNDED);

            let mut text: Vec<Line> = error.lines().map(Line::from).collect();
            text.push(Line::from(""));
            text.push(Line::from("Press any key to dismiss".dim()));

            let paragraph = Paragraph::new(text).block(block);
            let min_width = paragraph.line_width() as u16;
            let min_height = paragraph.line_count(min_width) as u16;

            let area = center(
                area,
                Constraint::Length(min_width + 4),
                Constraint::Length(min_height),
            );
            Clear.render(area, buf);

            paragraph.render(area, buf);
        }
        cursor
    }
}

impl WidgetWithCursor for Model {
    fn render_with_cursor(&self, area: Rect, buf: &mut Buffer) -> Option<Position> {
        let cursor = self.note_pane.render_with_cursor(area, buf);

        if let Some(search_window) = &self.search_window {
            let area = center(area, Constraint::Percentage(80), Constraint::Percentage(40));
            Clear.render(area, buf);
            return search_window.render_with_cursor(area, buf);
        }
        cursor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::MockRepository;
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn test_render_error() {
        let mut repository = MockRepository::new();
        let note = repository.insert_note("Note name.md", "This is a file.");
        let mut app = App::new(Box::new(repository), Some(note));

        app.error_message = Some(String::from("This is an error"));

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| {
                app.render_with_cursor(frame.area(), frame.buffer_mut());
            })
            .unwrap();
        assert_snapshot!(terminal.backend());
    }
}
