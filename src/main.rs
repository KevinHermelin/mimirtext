mod file_buffer;
mod markdown_view;
mod repository;

use clap::{Parser, command};
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    crossterm::{
        ExecutableCommand,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::Stylize,
    symbols::border,
    text::Line,
    widgets::{Block, Clear, Paragraph, Widget},
};
use std::{
    cmp::max,
    io::stdout,
    path::{Path, PathBuf},
};

use crate::{
    markdown_view::{MarkdownDocument, MarkdownView},
    repository::{FolderRepository, Note, Repository},
};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    file_path: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let file_path = cli.file_path;
    let mut app = App::from_path(&file_path);

    color_eyre::install()?;
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

pub struct App {
    repository: Box<dyn Repository>,
    note: Box<dyn Note>,
    scroll_lines: i16,
    link_selection_index: isize,
    should_exit: bool,
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
    fn new(repository: Box<dyn Repository>, note: Box<dyn Note>) -> Self {
        return App {
            repository,
            note,
            scroll_lines: 0,
            link_selection_index: 0,
            should_exit: false,
            should_edit: false,
            error_message: None,
            navigation_history: vec![],
        };
    }
    fn from_path(path: &Path) -> Self {
        let (repo, note) = FolderRepository::open_path(&path);
        let repo = repo.expect("path should point to note inside valid repository");
        let note = note.expect("path should point to valid note");

        App::new(Box::new(repo), Box::new(note))
    }
    fn get_document(&self) -> MarkdownDocument {
        let mut document = MarkdownDocument::new(&self.note.content());
        document.selected_link = document
            .get_links()
            .get::<usize>(self.link_selection_index.try_into().unwrap())
            .cloned();
        document
    }
    fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_event()?;

            if self.should_edit {
                self.edit_file(terminal)
                    .expect("should be able to edit file");
                self.should_edit = false;
            }
        }
        Ok(())
    }
    fn edit_file(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        stdout().execute(LeaveAlternateScreen)?;
        disable_raw_mode()?;
        self.note.edit_externally()?;
        stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;
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
            Action::Exit => self.should_exit = true,
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
        let number_of_lines: i16 = self.note.content().lines().count().try_into().unwrap();

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
        match self.get_document().selected_link {
            None => return,
            Some(link_ref) => {
                let old_id = self.note.id().to_owned();
                let new_id = self.repository.resolve_reference(&link_ref.target);
                let success = self.open_path(&new_id);

                if success {
                    self.navigation_history.push(old_id);
                }
            }
        }
    }
    fn open_path(&mut self, id: &str) -> bool {
        let new_note = self.repository.get_note(id);
        match new_note {
            Err(_) => {
                self.error_message = Some(format!(
                    "Could not open \"{:?}\" in current repository.",
                    id
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
        let title = Line::from(self.note.name().to_string().bold());
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

        if self.note.content() == "" {
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
            let text = error.reset();
            let helper_text = "Press any key to dismiss".dim();
            let min_width = max(text.width(), helper_text.width()) as u16;

            let area = center(
                area,
                Constraint::Length(min_width + 6),
                Constraint::Length(5),
            );
            Clear.render(area, buf);
            let block = Block::bordered().title("Error").border_set(border::ROUNDED);

            Paragraph::new(vec![
                Line::from(text),
                Line::from(""),
                Line::from(helper_text),
            ])
            .block(block)
            .render(area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::InMemoryRepository;
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn test_render_buffer() {
        let mut repository = InMemoryRepository::default();
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
        let mut repository = InMemoryRepository::default();
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
        let mut repository = InMemoryRepository::default();
        let note = repository.get_note("nonexistent.md").unwrap();
        let app = App::new(Box::new(repository), note);

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&app, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_handle_scroll_action() {
        let mut repository = InMemoryRepository::default();
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
        let mut repository = InMemoryRepository::default();
        let note = repository.insert_note("Note name.md", "this is a test file");
        let mut app = App::new(Box::new(repository), note);

        app.handle_action(Action::Exit);
        assert_eq!(app.should_exit, true);
    }

    #[test]
    fn test_handle_link_selection() {
        let mut repository = InMemoryRepository::default();
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

        app.open_path(note_without_link.id());
        assert_eq!(app.link_selection_index, 0);

        app.handle_action(Action::NextLink);
        assert_eq!(app.link_selection_index, 0);
    }

    #[test]
    fn test_handle_link_follow() {
        let mut repository = InMemoryRepository::default();
        let file_a = repository.insert_note(
            "file_a.md",
            "[[file_a|This]] contains two [[file_b|links]].",
        );
        repository.insert_note("file_b.md", "This is the other file");
        let mut app = App::new(Box::new(repository), file_a);

        app.link_selection_index = 1;
        app.scroll_lines = 1;

        app.handle_action(Action::FollowLink);

        assert_eq!(app.note.id(), "file_b.md");
        assert_eq!(app.note.name(), "file_b.md");
        assert_eq!(app.note.content(), "This is the other file");

        // Link selection and scroll should be reset.
        assert_eq!(app.scroll_lines, 0);
        assert_eq!(app.link_selection_index, 0);
    }

    #[test]
    fn test_handle_edit_file() {
        let mut repository = InMemoryRepository::default();
        let note = repository.insert_note("Note name.md", "this is a test file");
        let mut app = App::new(Box::new(repository), note);

        app.handle_action(Action::EditFile);
        assert_eq!(app.should_edit, true)
    }

    #[test]
    fn test_navigation_history() {
        let mut repository = InMemoryRepository::default();
        let file_a = repository.insert_note("file_a.md", "This has a link to [[file_b]].");
        repository.insert_note("file_b.md", "This has a link to [[file_c]]");
        repository.insert_note("file_c.md", "This has no links");
        let mut app = App::new(Box::new(repository), file_a);

        assert_eq!(app.note.id(), "file_a.md");
        app.handle_action(Action::FollowLink);
        assert_eq!(app.note.id(), "file_b.md");
        app.handle_action(Action::FollowLink);
        assert_eq!(app.note.id(), "file_c.md");
        app.handle_action(Action::NavigateBack);
        assert_eq!(app.note.id(), "file_b.md");
        app.handle_action(Action::NavigateBack);
        assert_eq!(app.note.id(), "file_a.md");
        app.handle_action(Action::NavigateBack);
        assert_eq!(app.note.id(), "file_a.md");
    }
}
