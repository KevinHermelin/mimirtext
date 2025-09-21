mod markdown_view;
mod model;
mod repository;

use clap::{Parser, command};
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    Frame, Terminal,
    buffer::Buffer,
    crossterm::{
        ExecutableCommand,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    prelude::Backend,
    style::Stylize,
    symbols::border,
    text::Line,
    widgets::{Block, Clear, Paragraph, Widget},
};
use std::{
    cmp::max,
    io::{self, ErrorKind, stdout},
    path::{Path, PathBuf},
};

use crate::{
    markdown_view::{LinkTarget, MarkdownDocument, MarkdownView},
    model::{Message, Model, RunningState, Update},
    repository::{FolderRepository, NoteSnapshot, Repository},
};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    file_path: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let file_path = cli.file_path;
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
    note: NoteSnapshot,
    scroll_lines: i16,
    link_selection_index: isize,
    should_edit: bool,
    error_message: Option<String>,
    navigation_history: Vec<String>,
}

enum Action {
    ScrollUp,
    ScrollDown,
    Exit,
    None,
    NextLink,
    PreviousLink,
    FollowLink,
    NavigateBack,
    DismissError,
    EditFile,
}

impl App {
    fn new(repository: Box<dyn Repository>, note: NoteSnapshot) -> Self {
        return App {
            model: Model::default(),
            repository,
            note,
            scroll_lines: 0,
            link_selection_index: 0,
            should_edit: false,
            error_message: None,
            navigation_history: vec![],
        };
    }
    fn from_path(path: &Path) -> io::Result<Self> {
        let (repo, note) = FolderRepository::open_path(&path)?;
        let note = note.ok_or(io::Error::new(
            ErrorKind::Other,
            "Path does not resolve to a note file",
        ))?;

        Ok(App::new(Box::new(repo), note))
    }
    fn get_document(&self) -> MarkdownDocument {
        let mut document = MarkdownDocument::new(&self.note.body);
        document.selected_link = document
            .get_links()
            .get::<usize>(self.link_selection_index.try_into().unwrap())
            .cloned();
        document
    }
    fn run(&mut self, terminal: &mut Terminal<impl Backend>) -> Result<()> {
        while self.model.running_state != RunningState::Done {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_event()?;

            if self.should_edit {
                self.edit_file(terminal, &mut stdout(), &mut DefaultRawModeControl)
                    .expect("should be able to edit file");
                self.should_edit = false;
            }
        }
        Ok(())
    }
    fn edit_file(
        &mut self,
        terminal: &mut Terminal<impl Backend>,
        executor: &mut impl ExecutableCommand,
        raw_mode_control: &mut impl RawModeControl,
    ) -> Result<()> {
        executor.execute(LeaveAlternateScreen)?;
        raw_mode_control.disable_raw_mode()?;
        let id = self.note.key.note_id();
        self.repository.edit_externally(id)?;
        self.note = self.repository.note(id)?;
        executor.execute(EnterAlternateScreen)?;
        raw_mode_control.enable_raw_mode()?;
        terminal.clear()?;
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
        if let Some(_) = self.error_message {
            self.handle_action(Action::DismissError);
            return;
        }

        let action = match key_event.code {
            KeyCode::Down => Action::ScrollDown,
            KeyCode::Up => Action::ScrollUp,
            KeyCode::Right => Action::NextLink,
            KeyCode::Left => Action::PreviousLink,
            KeyCode::Enter => Action::FollowLink,
            KeyCode::Backspace => Action::NavigateBack,
            KeyCode::Char('q') => Action::Exit,
            KeyCode::Char('c') => Action::EditFile,
            _ => Action::None,
        };
        self.handle_action(action);
    }
    fn handle_action(&mut self, action: Action) {
        match action {
            Action::ScrollDown => self.move_scroll(1),
            Action::ScrollUp => self.move_scroll(-1),
            Action::Exit => self.model = self.model.update(Message::Quit).0,
            Action::NextLink => self.move_link_selection(1),
            Action::PreviousLink => self.move_link_selection(-1),
            Action::FollowLink => self.follow_link(),
            Action::DismissError => self.error_message = None,
            Action::NavigateBack => self.navigate_back(),
            Action::EditFile => self.should_edit = true,
            Action::None => {}
        }
    }
    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(Clear, frame.area());
        frame.render_widget(self, frame.area());
    }
    fn move_scroll(&mut self, lines: i16) {
        let number_of_lines: i16 = self.note.body.lines().count().try_into().unwrap();

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
    fn follow_link(&mut self) {
        if let Some(link_ref) = self.get_document().selected_link {
            match link_ref.target {
                LinkTarget::Note(target) => {
                    let old_id = self.note.key.note_id().to_owned();
                    let new_id = self.repository.resolve_reference(&target);
                    let success = self.open_path(&new_id);

                    if success {
                        self.navigation_history.push(old_id);
                    }
                }
                LinkTarget::External(target) => {
                    let result = open::that(&target);
                    if let Err(error) = result {
                        self.error_message =
                            Some(format!("Could not open link\n{}\nGot: {}", target, error));
                    }
                }
            }
        }
    }
    fn open_path(&mut self, id: &str) -> bool {
        let new_note = self.repository.note(id);
        match new_note {
            Err(error) => {
                self.error_message = Some(format!(
                    "Could not open \"{}\" in current repository\nGot: {}",
                    id, error
                ));
                false
            }
            Ok(new_note) => {
                self.note = new_note;
                self.link_selection_index = 0;
                self.scroll_lines = 0;
                true
            }
        }
    }
    fn navigate_back(&mut self) {
        if let Some(previous) = self.navigation_history.last().cloned() {
            if self.open_path(&previous) {
                self.navigation_history.pop();
            }
        }
    }
}

fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
    let [area] = Layout::horizontal([horizontal])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);
    area
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from(self.note.title.to_string().bold());
        let instructions = Line::from(vec![" Exit ".into(), "<Q> ".blue().bold()]);
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.right_aligned())
            .border_set(border::THICK);
        let inner_area = block.inner(area);

        block.render(area, buf);

        let document = self.get_document();

        MarkdownView::new(document)
            .scroll(self.scroll_lines.try_into().unwrap())
            .render(inner_area, buf);

        if self.note.body.is_empty() {
            let text = "This note is empty".bold();
            let helper_text = "Press <C> to open in editor".dim();
            let min_width = max(text.width(), helper_text.width()) as u16;

            let area = center(area, Constraint::Length(min_width), Constraint::Length(5));

            Paragraph::new(vec![
                Line::from(text),
                Line::from(""),
                Line::from(helper_text),
            ])
            .alignment(Alignment::Center)
            .render(area, buf);
        }

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
    }
}

#[cfg(test)]
struct NoExecution;

#[cfg(test)]
impl ExecutableCommand for NoExecution {
    fn execute(&mut self, _: impl ratatui::crossterm::Command) -> io::Result<&mut Self> {
        Ok(self)
    }
}

trait RawModeControl {
    fn disable_raw_mode(&mut self) -> io::Result<()>;
    fn enable_raw_mode(&mut self) -> io::Result<()>;
}

struct DefaultRawModeControl;

impl RawModeControl for DefaultRawModeControl {
    fn disable_raw_mode(&mut self) -> io::Result<()> {
        disable_raw_mode()
    }

    fn enable_raw_mode(&mut self) -> io::Result<()> {
        enable_raw_mode()
    }
}

#[cfg(test)]
struct TestRawModeControl;

#[cfg(test)]
impl RawModeControl for TestRawModeControl {
    fn disable_raw_mode(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn enable_raw_mode(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::MockRepository;
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn test_render_buffer() {
        let mut repository = MockRepository::new();
        let note = repository.insert_note(
            "Note name.md",
            "this should not be visible.\n\nthis is a test file\n\nwith multiple\n\nparagraphs",
        );
        let mut app = App::new(Box::new(repository), note);

        app.scroll_lines = 1;

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&app, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_render_error() {
        let mut repository = MockRepository::new();
        let note = repository.insert_note("Note name.md", "This is a file.");
        let mut app = App::new(Box::new(repository), note);

        app.error_message = Some(String::from("This is an error"));

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&app, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());

        app.handle_action(Action::DismissError);
        assert_eq!(app.error_message, None);
    }

    #[test]
    fn test_render_new_file() {
        let repository = MockRepository::new();
        let note = repository.note("nonexistent.md").unwrap();
        let app = App::new(Box::new(repository), note);

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&app, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_handle_scroll_action() {
        let mut repository = MockRepository::new();
        let note = repository.insert_note(
            "Note name.md",
            "this is a test file\nspanning multiple\nlines",
        );
        let mut app = App::new(Box::new(repository), note);

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
        let mut repository = MockRepository::new();
        let note = repository.insert_note("Note name.md", "this is a test file");
        let mut app = App::new(Box::new(repository), note);

        assert_eq!(app.model.running_state, RunningState::Running);
        app.handle_action(Action::Exit);
        assert_eq!(app.model.running_state, RunningState::Done);
    }

    #[test]
    fn test_handle_link_selection() {
        let mut repository = MockRepository::new();
        let note_with_link = repository.insert_note(
            "Note name.md",
            "[[This]] file [[Have|has]] multiple [links](url.com)",
        );
        let note_without_link = repository.insert_note("Note name.md", "This file has no link");
        let mut app = App::new(Box::new(repository), note_with_link);

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

        app.open_path(note_without_link.key.note_id());
        assert_eq!(app.link_selection_index, 0);

        app.handle_action(Action::NextLink);
        assert_eq!(app.link_selection_index, 0);
    }

    #[test]
    fn test_handle_link_follow() {
        let mut repository = MockRepository::new();

        let note_a = repository.insert_note(
            "note A.md",
            "[[note A|This]] contains two [[note B|links]].",
        );
        let note_b = repository.insert_note("note B.md", "This is the other file");

        let mut app = App::new(Box::new(repository), note_a);

        app.link_selection_index = 1;
        app.scroll_lines = 1;

        app.handle_action(Action::FollowLink);

        assert_eq!(app.note, note_b);

        // Link selection and scroll should be reset.
        assert_eq!(app.scroll_lines, 0);
        assert_eq!(app.link_selection_index, 0);
    }

    #[test]
    fn test_handle_edit_file() {
        let mut repository = MockRepository::new();

        let note = repository.insert_note("Note name.md", "this is a test file");

        let mut app = App::new(Box::new(repository), note);

        app.handle_action(Action::EditFile);
        assert_eq!(app.should_edit, true)
    }

    #[test]
    fn test_edit_file() -> Result<()> {
        let mut repository = MockRepository::new();
        let note = repository.insert_note("Note name.md", "this has not been changed");

        repository.edit_externally_impl = Box::new(|mut note| {
            note.body = String::from("this has been changed");
            note
        });

        let mut terminal = Terminal::new(TestBackend::new(80, 20))?;
        let mut app = App::new(Box::new(repository), note);

        assert_eq!(app.note.body, "this has not been changed");
        app.edit_file(&mut terminal, &mut NoExecution, &mut TestRawModeControl)?;
        assert_eq!(app.note.body, "this has been changed");

        Ok(())
    }

    #[test]
    fn test_navigation_history() {
        let mut repository = MockRepository::new();

        let note_a = repository.insert_note("note A.md", "This has a link to [[note B]].");
        let note_b = repository.insert_note("note B.md", "This has a link to [[note C]]");
        let note_c = repository.insert_note("note C.md", "This has no links");

        let mut app = App::new(Box::new(repository), note_a.clone());

        assert_eq!(app.note, note_a);
        app.handle_action(Action::FollowLink);
        assert_eq!(app.note, note_b);
        app.handle_action(Action::FollowLink);
        assert_eq!(app.note, note_c);
        app.handle_action(Action::NavigateBack);
        assert_eq!(app.note, note_b);
        app.handle_action(Action::NavigateBack);
        assert_eq!(app.note, note_a);
        app.handle_action(Action::NavigateBack);
        assert_eq!(app.note, note_a);
    }
}
