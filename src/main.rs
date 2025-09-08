mod file_buffer;
mod markdown_view;

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
    should_edit: bool,
    error_message: Option<String>,
    navigation_history: Vec<PathBuf>,
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
    fn new(buffer: FileBuffer) -> Self {
        return App {
            buffer,
            scroll_lines: 0,
            link_selection_index: 0,
            should_exit: false,
            should_edit: false,
            error_message: None,
            navigation_history: vec![],
        };
    }
    fn get_document(&self) -> MarkdownDocument {
        let mut document = MarkdownDocument::new(&self.buffer.content);
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
        edit::edit_file(&self.buffer.file_path)?;
        self.open_path(&self.buffer.file_path.to_owned());
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
    fn follow_link(&mut self) {
        match self.get_document().selected_link {
            None => return,
            Some(link_ref) => {
                let old_path = self.buffer.file_path.to_owned();
                let repository_root = self
                    .buffer
                    .file_path
                    .parent()
                    .expect("opened file should have a parent");

                // TODO: Target might be an URL.
                let target_filename = format!("{}.md", link_ref.target);
                let target = repository_root.join(&target_filename);

                let success = self.open_path(&target);

                if success {
                    self.navigation_history.push(old_path);
                }
            }
        }
    }
    fn open_path(&mut self, target: &Path) -> bool {
        let new_buffer = FileBuffer::from_file(target);
        match new_buffer {
            Err(_) => {
                self.error_message = Some(format!(
                    "Could not open {:?} in current repository.",
                    target
                        .file_name()
                        .expect("should be able to parse file name")
                ));
                false
            }
            Ok(new_buffer) => {
                self.buffer = new_buffer;
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

        let document = self.get_document();

        MarkdownView::new(document)
            .scroll(self.scroll_lines.try_into().unwrap())
            .render(inner_area, buf);

        if !self.buffer.created {
            let text = "This file has not been created yet".bold();
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
    use std::{fs::File, io::Write};

    use super::*;
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend};
    use tempfile::TempDir;

    #[test]
    fn test_render_buffer() {
        let mut app = App::new(FileBuffer::mock(
            "this should not be visible.\n\nthis is a test file\n\nwith multiple\n\nparagraphs",
        ));
        app.scroll_lines = 1;

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&app, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_render_error() {
        let mut app = App::new(FileBuffer::mock("This is a file."));

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
        let app = App::new(
            FileBuffer::from_file(&PathBuf::from("nonexistent.md"))
                .expect("should be able to create FileBuffer for non-existent file"),
        );

        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(&app, frame.area()))
            .unwrap();
        assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_handle_scroll_action() {
        let mut app = App::new(FileBuffer::mock(
            "this is a test file\nspanning multiple\nlines",
        ));

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
        let mut app = App::new(FileBuffer::mock(
            "this is a test file\nspanning multiple\nlines",
        ));

        app.handle_action(Action::Exit);
        assert_eq!(app.should_exit, true);
    }

    #[test]
    fn test_handle_link_selection() {
        let mut app = App::new(FileBuffer::mock(
            "[[This]] file [[Have|has]] multiple [links](url.com)",
        ));

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

        let mut app = App::new(FileBuffer::mock("This file has no link"));

        assert_eq!(app.link_selection_index, 0);

        app.handle_action(Action::NextLink);
        assert_eq!(app.link_selection_index, 0);
    }

    #[test]
    fn test_handle_link_follow() {
        let temp_dir = TempDir::new().expect("should be able to create temporary directory");

        let file_a_path = temp_dir.path().join("file_a.md");
        let file_b_path = temp_dir.path().join("file_b.md");

        let mut file_a =
            File::create(&file_a_path).expect("should be able to create temporary file");
        let mut file_b =
            File::create(&file_b_path).expect("should be able to create temporary file");

        write!(
            file_a,
            "
# Link test
[[file_a|This]] contains two [[file_b|links]].
"
        )
        .expect("should be able to write to temporary file");
        write!(file_b, "This is the other file")
            .expect("should be able to write to temporary file");

        let mut app = App::new(
            FileBuffer::from_file(&file_a_path).expect("should be able to read temporary file"),
        );
        app.link_selection_index = 1;
        app.scroll_lines = 1;

        app.handle_action(Action::FollowLink);

        assert_eq!(app.buffer.file_path, file_b_path);
        assert_eq!(app.buffer.content, "This is the other file");

        // Link selection and scroll should be reset.
        assert_eq!(app.scroll_lines, 0);
        assert_eq!(app.link_selection_index, 0);
    }

    #[test]
    fn test_handle_edit_file() {
        let mut app = App::new(FileBuffer::mock("This is a file"));
        app.handle_action(Action::EditFile);
        assert_eq!(app.should_edit, true)
    }

    #[test]
    fn test_navigation_history() {
        let temp_dir = TempDir::new().expect("should be able to create temporary directory");

        let file_a_path = temp_dir.path().join("file_a.md");
        let file_b_path = temp_dir.path().join("file_b.md");
        let file_c_path = temp_dir.path().join("file_c.md");

        let mut file_a =
            File::create(&file_a_path).expect("should be able to create temporary file");
        let mut file_b =
            File::create(&file_b_path).expect("should be able to create temporary file");
        let mut file_c =
            File::create(&file_c_path).expect("should be able to create temporary file");

        write!(file_a, "This has a link to [[file_b]]")
            .expect("should be able to write to temporary file");
        write!(file_b, "This has a link to [[file_c]]")
            .expect("should be able to write to temporary file");
        write!(file_c, "This has no links").expect("should be able to write to temporary file");

        let mut app = App::new(
            FileBuffer::from_file(&file_a_path).expect("should be able to read temporary file"),
        );

        assert_eq!(app.buffer.file_path, file_a_path);
        app.handle_action(Action::FollowLink);
        assert_eq!(app.buffer.file_path, file_b_path);
        app.handle_action(Action::FollowLink);
        assert_eq!(app.buffer.file_path, file_c_path);
        app.handle_action(Action::NavigateBack);
        assert_eq!(app.buffer.file_path, file_b_path);
        app.handle_action(Action::NavigateBack);
        assert_eq!(app.buffer.file_path, file_a_path);
        app.handle_action(Action::NavigateBack);
        assert_eq!(app.buffer.file_path, file_a_path);
    }
}
