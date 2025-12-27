mod input;
pub mod markdown_view;
mod note_pane;
mod search_window;
pub mod utils;

use crate::{
    document::LinkTarget,
    graph::RepositoryGraph,
    model::{
        Command, Message, Model, RunningState, Update, note_pane::NotePaneMessage,
        search_window::SearchWindowMessage,
    },
    repository::{NoteKey, NoteSnapshot, Repository, folder::FolderRepository, resolve_label},
    text_input::Completion,
    tui::{input::KeyHandler, utils::center},
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
    error, fmt,
    io::{self, stdout},
    path::{Path, PathBuf},
    sync::{
        Arc, RwLock,
        mpsc::{self, Receiver, Sender},
    },
    thread,
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
    error_message: Option<String>,
}

pub trait WidgetWithCursor {
    fn render_with_cursor(&self, area: Rect, buf: &mut Buffer) -> Option<Position>;
}

impl App {
    fn new(
        repository: Arc<RwLock<dyn Repository + Send + Sync>>,
        note: Option<NoteSnapshot>,
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
            error_message: None,
        }
    }
    fn from_path(path: &Path) -> io::Result<Self> {
        let (repo, note) = FolderRepository::open_path(path)?;

        Ok(App::new(Arc::new(RwLock::new(repo)), note))
    }
    fn run(&mut self, terminal: &mut Terminal<impl Backend>) -> Result<()> {
        let (graph_progress_tx, graph_progress_rx) = mpsc::channel();

        let graph = Arc::new(RwLock::new(RepositoryGraph::new()));

        {
            let repository = Arc::clone(&self.repository);
            let graph = Arc::clone(&graph);
            thread::spawn(move || {
                // An error in the index thread should make the main thread panic.
                build_graph(repository, graph, graph_progress_tx).unwrap();
            });
        }

        let mut message = Message::None;
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

#[derive(Debug)]
enum GraphThreadError {
    RepositoryList(String, io::Error),
    NoteParse(NoteKey, io::Error),
}

impl fmt::Display for GraphThreadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::RepositoryList(repo_id, ..) => {
                write!(f, "could not list notes from repository {:?}", repo_id)
            }
            Self::NoteParse(note_key, ..) => {
                write!(f, "could not parse note {:?}", note_key)
            }
        }
    }
}

impl error::Error for GraphThreadError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::RepositoryList(_, e) => Some(e),
            Self::NoteParse(_, e) => Some(e),
        }
    }
}

/// Represents the progress of a graph building job.
///
/// First index is the number of files indexed.
/// Second index is the total number of files to index.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphBuildProgress(pub usize, pub usize);

impl GraphBuildProgress {
    /// Calculates percentage done, as a float between 0.0 and 1.0.
    pub fn percentage(&self) -> f32 {
        let GraphBuildProgress(done, total) = self;
        let done: f32 = *done as f32;
        let total: f32 = *total as f32;
        done / total
    }
    /// `true` if the job is complete, i.e. all files have been indexed.
    pub fn done(&self) -> bool {
        let GraphBuildProgress(done, total) = self;
        done == total
    }
}

fn build_graph(
    repository: Arc<RwLock<dyn Repository + Send + Sync + 'static>>,
    graph: Arc<RwLock<RepositoryGraph>>,
    graph_progress_tx: Sender<GraphBuildProgress>,
) -> Result<(), GraphThreadError> {
    // Important that repository is not locked for later.
    let repo_id = repository.read().unwrap().id().to_string();
    let queue = repository
        .read()
        .unwrap()
        .notes()
        .map_err(|error| GraphThreadError::RepositoryList(repo_id, error))?
        .clone();

    let notes_count = queue.len();

    for (i, key) in queue.iter().enumerate() {
        let NoteKey(_, id) = key.clone();

        let note = repository
            .read()
            .unwrap()
            .note(&id)
            .map_err(|error| GraphThreadError::NoteParse(key.clone(), error));

        // Silently ignoring all files which cannot be read. Otherwise, this would panic on pictures in the repo.
        // TODO: This should be better handled.
        if let Ok(note) = note {
            graph.write().unwrap().register_note(&note);
        }

        graph_progress_tx
            .send(GraphBuildProgress(i + 1, notes_count))
            .expect("should be able to send graph progress");
    }

    Ok(())
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

    #[test]
    fn test_render_error() {
        let mut repository = MockRepository::new();
        let note = repository.insert_note("Note name.md", "This is a file.");
        let mut app = App::new(Arc::new(RwLock::new(repository)), Some(note));

        app.error_message = Some(String::from("This is an error"));

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| {
                app.render_with_cursor(frame.area(), frame.buffer_mut());
            })
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_graph_build_progress_percentage() {
        assert_eq!(GraphBuildProgress(10, 100).percentage(), 0.1);
        assert_eq!(GraphBuildProgress(5, 10).percentage(), 0.5);
        assert_eq!(GraphBuildProgress(1, 4).percentage(), 0.25);
    }

    #[test]
    fn test_graph_build_progress_done() {
        assert!(!GraphBuildProgress(10, 100).done());
        assert!(!GraphBuildProgress(0, 5).done());
        assert!(!GraphBuildProgress(99, 100).done());
        assert!(GraphBuildProgress(10, 10).done());
    }
}
