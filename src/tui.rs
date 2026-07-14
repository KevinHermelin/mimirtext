mod input;
pub mod markdown_view;
mod note_pane;
mod search_window;
pub mod utils;

use crate::{
    document::LinkTarget,
    graph::{
        RepositoryGraph,
        builder::{GraphBuildProgress, create_graph_builder},
    },
    model::{
        Command, Message, Model, RunningState, Update,
        note_pane::NotePaneMessage::{self, GitStatusUpdate},
        search_window::SearchWindowMessage,
    },
    repository::{NoteKey, NoteSnapshot, Repository, folder::FolderRepository, resolve_label},
    text_input::Completion,
    tui::{input::KeyHandler, utils::center},
    upstream::{Git, GitShell},
};
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
    sync::{
        Arc, RwLock,
        mpsc::{self, Receiver},
    },
    time::Duration,
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
    repository: Arc<RwLock<dyn Repository + Send + Sync>>,
    upstream: Box<dyn Git>,
    error_message: Option<String>,
}

pub trait WidgetWithCursor {
    fn render_with_cursor(&self, area: Rect, buf: &mut Buffer) -> Option<Position>;
}

impl App {
    fn new(
        repository: Arc<RwLock<dyn Repository + Send + Sync>>,
        note: Option<NoteSnapshot>,
        upstream: Box<dyn Git>,
    ) -> Self {
        let mut model = Model::new(repository.read().unwrap().id());

        if let Some(note) = note {
            let (new_model, command) =
                model.update(Message::NotePane(NotePaneMessage::PushNote(note)));
            model = new_model;
            assert_eq!(command, Command::None);
        }

        App {
            model,
            repository,
            upstream,
            error_message: None,
        }
    }
    fn from_path(path: &Path) -> io::Result<Self> {
        let (repo, note) = FolderRepository::open_path(path)?;
        let repo = Arc::new(RwLock::new(repo));

        let git = GitShell::new(repo.clone());

        Ok(App::new(repo, note, Box::new(git)))
    }
    fn run(&mut self, terminal: &mut Terminal<impl Backend>) -> Result<()> {
        let (graph_progress_tx, graph_progress_rx) = mpsc::channel();

        let graph = Arc::new(RwLock::new(RepositoryGraph::new()));

        create_graph_builder(
            Arc::clone(&self.repository),
            Arc::clone(&graph),
            graph_progress_tx,
        );

        let mut message = Message::NotePane(GitStatusUpdate(self.upstream.get_status()));

        while self.model.running_state != RunningState::Done {
            terminal.draw(|frame| self.draw(frame))?;

            if let Message::None = message {
                message = self.handle_event(&graph_progress_rx)?;
            }

            let (model, command) = self.model.update(message);
            message = Message::None;
            self.model = model;

            match command {
                Command::EditExternally(note) => {
                    let mut repository = self.repository.write().unwrap();

                    stdout().execute(LeaveAlternateScreen)?;
                    disable_raw_mode()?;

                    let NoteKey(repo_id, note_id) = note;
                    assert_eq!(repo_id, repository.id());

                    repository.edit_externally(&note_id)?;
                    let note = repository.note(&note_id)?;
                    message = Message::NotePane(NotePaneMessage::UpdateNote(note));

                    stdout().execute(EnterAlternateScreen)?;
                    enable_raw_mode()?;
                    terminal.clear()?;
                }
                Command::FollowLink(link) => match link {
                    LinkTarget::Note(target) => {
                        let repository = self.repository.read().unwrap();

                        let id = resolve_label(&target);
                        let note = repository.note(&id)?;
                        message = Message::NotePane(NotePaneMessage::PushNote(note));
                    }
                    LinkTarget::External(target) => {
                        open::that(&target)?;
                    }
                },
                Command::ServeNote(note) => {
                    let repository = self.repository.read().unwrap();

                    let NoteKey(repo_id, id) = note;
                    assert_eq!(repo_id, repository.id());
                    let note = repository.note(&id)?;
                    message = Message::NotePane(NotePaneMessage::UpdateNote(note));
                }
                Command::SearchQuery(query) => {
                    let results = graph.read().unwrap().search(&query);
                    message = Message::SearchWindow(SearchWindowMessage::UpdateResults(results));
                }
                Command::OpenNote(key) => {
                    let repository = self.repository.read().unwrap();

                    let NoteKey(_, note_id) = key;
                    let note = repository.note(&note_id)?;
                    message = Message::NotePane(NotePaneMessage::PushNote(note));
                }
                Command::CommitNote(note) => {
                    let mut repository = self.repository.write().unwrap();

                    let NoteKey(repo_id, note_id) = &note.key.clone();
                    assert_eq!(repo_id, repository.id());
                    repository.commit_note(note)?;

                    let note = repository.note(note_id)?;
                    graph.write().unwrap().register_note(&note);
                    message = Message::NotePane(NotePaneMessage::UpdateNote(note));
                }
                Command::RequestLinkCompletion(range, text) => {
                    let results = graph.read().unwrap().search(&text);

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
    /// Listens for events from input or from `graph_progress_rx` and converts them
    /// into messages.
    ///
    /// This function waits at most 100 ms for user input.
    fn handle_event(
        &mut self,
        graph_progress_rx: &Receiver<GraphBuildProgress>,
    ) -> Result<Message> {
        // Do not block UI waiting for key input.
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    return Ok(self.handle_key_event(key_event));
                }
                _ => {}
            }
        }

        if let Some(progress) = graph_progress_rx.try_iter().last() {
            return Ok(Message::NotePane(NotePaneMessage::GraphUpdate(progress)));
        }

        Ok(Message::None)
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
    use crate::repository::mock::MockRepository;
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend};

    struct MockUpstream {}

    impl Git for MockUpstream {
        fn head_name(&self) -> Option<String> {
            None
        }
    }

    #[test]
    fn test_render_error() {
        let mut repository = MockRepository::new();
        let note = repository.insert_note("Note name.md", "This is a file.");
        let mut app = App::new(
            Arc::new(RwLock::new(repository)),
            Some(note),
            Box::new(MockUpstream {}),
        );

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
