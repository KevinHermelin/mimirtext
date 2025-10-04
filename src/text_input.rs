use std::ops::Range;

use unicode_segmentation::GraphemeCursor;
use unicode_width::UnicodeWidthStr;

use crate::selection::NonEmptySelection;

#[derive(Clone, Debug, PartialEq)]
pub struct Completion {
    pub range: Range<usize>,
    pub full_text: String,
    pub display_text: String,
}

impl Completion {
    pub fn note_link(range: Range<usize>, note_reference: &str) -> Self {
        Self {
            range,
            display_text: note_reference.to_owned(),
            full_text: format!("[[{}]]", note_reference),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextInputConfig {
    pub tab_columns: usize,
}

impl Default for TextInputConfig {
    fn default() -> Self {
        Self { tab_columns: 2 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextInput {
    current: String,
    cursor_pos: usize,
    desired_column: usize,
    completions: Option<NonEmptySelection<Completion>>,
    config: TextInputConfig,
}

enum Movement {
    NextBoundary,
    PreviousBoundary,
}

impl From<&str> for TextInput {
    fn from(value: &str) -> Self {
        let mut input = Self::new();
        input.current = value.to_owned();
        input
    }
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            current: String::new(),
            cursor_pos: 0,
            desired_column: 0,
            completions: None,
            config: TextInputConfig::default(),
        }
    }
    #[cfg(test)]
    pub fn new_with(text: &str, cursor_pos: usize) -> Self {
        Self {
            current: text.to_owned(),
            cursor_pos,
            desired_column: 0,
            completions: None,
            config: TextInputConfig::default(),
        }
    }
    pub fn with_config(mut self, config: TextInputConfig) -> Self {
        self.config = config;
        self
    }
    fn cursor(&self) -> GraphemeCursor {
        GraphemeCursor::new(self.cursor_pos, self.current.len(), true)
    }
    pub fn text(&self) -> String {
        self.current.clone()
    }
    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }
    pub fn completions(&self) -> &Option<NonEmptySelection<Completion>> {
        &self.completions
    }
    pub fn cursor_row(&self) -> usize {
        self.current[..self.cursor_pos].split('\n').count() - 1
    }
    pub fn cursor_column(&self) -> usize {
        let current_line = self.current[..self.cursor_pos]
            .split('\n')
            .last()
            .expect("should have at least one line");

        assert!(
            self.config.tab_columns >= 1,
            "tabs cannot be less than 1 in width"
        );
        current_line.width() + current_line.matches('\t').count() * (self.config.tab_columns - 1)
    }
    pub fn before_cursor(&self) -> &str {
        &self.current[..self.cursor_pos]
    }
    fn move_cursor(&self, movement: Movement) -> Option<usize> {
        let mut cursor = self.cursor();
        let chunk = &self.current;
        let chunk_start = 0;

        match movement {
            Movement::NextBoundary => cursor.next_boundary(chunk, chunk_start),
            Movement::PreviousBoundary => cursor.prev_boundary(chunk, chunk_start),
        }
        .expect("chunk should be complete")
    }
    pub fn apply(mut self, operation: InputOperation) -> Self {
        // This ensures that we are not left with any completions on the old buffer,
        // that would otherwise cause issues.
        let completions = self.completions;
        self.completions = None;

        match operation {
            InputOperation::Insert(text) => {
                self.current.insert_str(self.cursor().cur_cursor(), &text);
                self.cursor_pos = self.cursor().cur_cursor() + text.len();
                self.desired_column = self.cursor_column();
            }
            InputOperation::Backspace => {
                let current_pos = self.cursor().cur_cursor();
                if let Some(new_pos) = self.move_cursor(Movement::PreviousBoundary) {
                    self.cursor_pos = new_pos;
                    self.current.drain(new_pos..current_pos);
                    self.desired_column = self.cursor_column();
                }
            }
            InputOperation::Left => {
                if let Some(new_pos) = self.move_cursor(Movement::PreviousBoundary) {
                    self.cursor_pos = new_pos;
                    self.desired_column = self.cursor_column();
                }
            }
            InputOperation::Right => {
                if let Some(new_pos) = self.move_cursor(Movement::NextBoundary) {
                    self.cursor_pos = new_pos;
                    self.desired_column = self.cursor_column();
                }
            }
            InputOperation::Up => {
                let prev_lb = self.current.as_bytes()[..self.cursor_pos]
                    .iter()
                    .rposition(|&b| b == b'\n');

                if let Some(prev_end) = prev_lb {
                    let prev_start = self.current.as_bytes()[..prev_end]
                        .iter()
                        .rposition(|&b| b == b'\n')
                        .map(|i| i + 1)
                        .unwrap_or(0);

                    self.cursor_pos = prev_start;
                    while self.cursor_column() < self.desired_column && self.cursor_pos < prev_end {
                        if let Some(p) = self.move_cursor(Movement::NextBoundary) {
                            self.cursor_pos = p
                        } else {
                            break;
                        }
                    }
                }
            }
            InputOperation::Down => {
                let next_lb = self.current.as_bytes()[self.cursor_pos..]
                    .iter()
                    .position(|&b| b == b'\n')
                    .map(|i| self.cursor_pos + i);

                if let Some(next_end) = next_lb {
                    let next_start = next_end + 1;
                    let next_line_end = self.current.as_bytes()[next_start..]
                        .iter()
                        .position(|&b| b == b'\n')
                        .map(|i| next_start + i)
                        .unwrap_or(self.current.as_bytes().len());

                    self.cursor_pos = next_start;
                    while self.cursor_column() < self.desired_column
                        && self.cursor_pos < next_line_end
                    {
                        if let Some(p) = self.move_cursor(Movement::NextBoundary) {
                            self.cursor_pos = p
                        } else {
                            break;
                        }
                    }
                } else {
                    // There is no next line.
                    self.cursor_pos = self.current.len();
                }
            }
            InputOperation::NextCompletion => {
                self.completions = completions.map(|completions| completions.next())
            }
            InputOperation::PreviousCompletion => {
                self.completions = completions.map(|completions| completions.previous())
            }
            InputOperation::Complete => {
                let selected_completion = completions
                    .as_ref()
                    .map(|completions| completions.selected().clone());

                if let Some(completion) = selected_completion {
                    // This will panic if start or end position of the completion is not located on a character boundary
                    // or if the positions are outside the current buffer. The responsibility to ensure that this does not
                    // happen will ultimately fall on the caller of this function.
                    let before = &self.current[..completion.range.start];
                    let after = &self.current[completion.range.end..];

                    self.cursor_pos = before.len() + completion.full_text.len();
                    self.current = before.to_owned() + &completion.full_text + after;
                }
            }
            InputOperation::None => {}
        };
        self
    }
    pub fn provide_completions(mut self, completions: Vec<Completion>) -> Self {
        if let Some(current_completions) = self.completions.clone() {
            // Do not disrupt any current selection if old and new completions are identical.
            if *current_completions.items() == completions {
                return self;
            }
        }
        self.completions = NonEmptySelection::new(completions);
        self
    }
}

#[derive(Clone)]
pub enum InputOperation {
    Insert(String),
    Backspace,
    Left,
    Right,
    Up,
    Down,
    None,
    NextCompletion,
    PreviousCompletion,
    Complete,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        assert_eq!(TextInput::new().text(), "");
    }

    #[test]
    fn test_from_str() {
        assert_eq!(TextInput::from("Test string").text(), "Test string");
    }

    #[test]
    fn test_cursor_pos() {
        assert_eq!(TextInput::new_with("Text", 0).cursor_pos(), 0);
        assert_eq!(TextInput::new_with("Text", 2).cursor_pos(), 2);
        assert_eq!(TextInput::new_with("Text", 3).cursor_pos(), 3);
    }

    #[test]
    fn test_cursor_column() {
        assert_eq!(TextInput::new().cursor_column(), 0);
        assert_eq!(
            TextInput::new()
                .apply(InputOperation::Insert(String::from("Test")))
                .cursor_column(),
            4
        );
        assert_eq!(
            TextInput::new()
                .apply(InputOperation::Insert(String::from("åäö")))
                .cursor_column(),
            3
        );
        assert_eq!(
            TextInput::new()
                .apply(InputOperation::Insert(String::from("åäö")))
                .apply(InputOperation::Left)
                .cursor_column(),
            2
        );
        assert_eq!(
            TextInput::new()
                .apply(InputOperation::Insert(String::from("Test\nNew")))
                .cursor_column(),
            3
        );

        // Depends on config.
        assert_eq!(
            TextInput::new()
                .apply(InputOperation::Insert(String::from("\tTest\n\t\tNew")))
                .cursor_column(),
            2 + 2 + 3
        );

        let mut config = TextInputConfig::default();
        config.tab_columns = 4;
        assert_eq!(
            TextInput::new()
                .with_config(config)
                .apply(InputOperation::Insert(String::from("\tTest\n\t\tNew")))
                .cursor_column(),
            4 + 4 + 3
        );
    }
    #[test]
    fn test_cursor_row() {
        assert_eq!(
            TextInput::new()
                .apply(InputOperation::Insert(String::from("")))
                .cursor_row(),
            0
        );

        assert_eq!(
            TextInput::new()
                .apply(InputOperation::Insert(String::from("Test\n")))
                .cursor_row(),
            1
        );

        assert_eq!(
            TextInput::new()
                .apply(InputOperation::Insert(String::from("Test\n")))
                .apply(InputOperation::Left)
                .cursor_row(),
            0
        );
    }

    #[test]
    fn test_before_cursor() {
        assert_eq!(
            TextInput::new()
                .apply(InputOperation::Insert(String::from("Test with one line")))
                .before_cursor(),
            "Test with one line"
        );

        assert_eq!(
            TextInput::new()
                .apply(InputOperation::Insert(String::from(
                    "Test with\nmultiple\nlines"
                )))
                .apply(InputOperation::Left)
                .before_cursor(),
            "Test with\nmultiple\nline"
        );

        assert_eq!(
            TextInput::new()
                .apply(InputOperation::Insert(String::from(
                    "Test with\nmultiple\nlines"
                )))
                .apply(InputOperation::Up)
                .apply(InputOperation::Left)
                .before_cursor(),
            "Test with\nmult"
        );
    }

    #[test]
    fn test_apply_none() {
        assert_eq!(
            TextInput::new(),
            TextInput::new().apply(InputOperation::None)
        );
    }

    #[test]
    fn test_apply_insert() {
        let input = TextInput::new()
            .apply(InputOperation::Insert(String::from("H")))
            .apply(InputOperation::Insert(String::from("el")))
            .apply(InputOperation::Insert(String::from("lo world")))
            .apply(InputOperation::Insert(String::from("!")));
        assert_eq!(input.text(), "Hello world!");
        assert_eq!(input.cursor_pos(), "Hello world!".len());

        let mut input = input;
        input.cursor_pos = "Hello ".len();

        // The emoji 👨‍👩‍👧‍👦 has multiple code points.
        let input = input.apply(InputOperation::Insert(String::from("👨‍👩‍👧‍👦 ")));
        assert_eq!(input.text(), "Hello 👨‍👩‍👧‍👦 world!");
        assert_eq!(input.cursor_pos(), "Hello 👨‍👩‍👧‍👦 ".len());

        // The emoji 👨‍👩 has three code points, we add them one by one.
        let input = TextInput::new()
            .apply(InputOperation::Insert(String::from("👨")))
            .apply(InputOperation::Insert(
                // Zero-width joiner
                char::from_u32(0x200D).unwrap().to_string(),
            ))
            .apply(InputOperation::Insert(String::from("👩")));

        assert_eq!(input.text(), "👨‍👩");
        assert_eq!(input.cursor_pos(), "👨‍👩".len())
    }

    #[test]
    fn test_apply_backspace() {
        let input = TextInput::new()
            .apply(InputOperation::Insert(String::from("Hello world!")))
            .apply(InputOperation::Backspace);
        assert_eq!(input.text(), "Hello world");
        assert_eq!(input.cursor_pos(), "Hello world".len());

        let mut input = input;
        input.cursor_pos = "Hello ".len();

        let input = input.apply(InputOperation::Backspace);
        assert_eq!(input.text(), "Helloworld");
        assert_eq!(input.cursor_pos(), "Hello".len());

        let mut input = input;
        input.cursor_pos = 0;

        let input = input.apply(InputOperation::Backspace);
        assert_eq!(input.text(), "Helloworld");
        assert_eq!(input.cursor_pos(), 0);

        // The emoji 👨‍👩‍👧‍👦 has multiple code points but should be removed by one backspace.
        let mut input =
            TextInput::new().apply(InputOperation::Insert(String::from("Hello 👨‍👩‍👧‍👦 world!")));
        input.cursor_pos = "Hello 👨‍👩‍👧‍👦".len();
        let input = input
            .apply(InputOperation::Backspace)
            .apply(InputOperation::Backspace);
        assert_eq!(input.text(), "Hello world!");
        assert_eq!(input.cursor_pos(), "Hello".len());
    }

    #[test]
    fn test_apply_movement() {
        let input = TextInput::new()
            .apply(InputOperation::Insert(String::from("ab 👨‍👩‍👧‍👦 cd")))
            .apply(InputOperation::Left)
            .apply(InputOperation::Left)
            .apply(InputOperation::Left)
            .apply(InputOperation::Left);
        assert_eq!(input.cursor_pos(), "ab ".len());

        let input = input
            .apply(InputOperation::Right)
            .apply(InputOperation::Right)
            .apply(InputOperation::Right);
        assert_eq!(input.cursor_pos(), "ab 👨‍👩‍👧‍👦 c".len());

        let input = TextInput::new()
            .apply(InputOperation::Insert(String::from("a")))
            .apply(InputOperation::Left)
            .apply(InputOperation::Left);
        assert_eq!(input.cursor_pos(), 0);

        let input = TextInput::new()
            .apply(InputOperation::Insert(String::from("a")))
            .apply(InputOperation::Right);
        assert_eq!(input.cursor_pos(), "a".len());
    }

    #[test]
    fn test_apply_multiline_movement() {
        let input =
            TextInput::new().apply(InputOperation::Insert(String::from("Hello\nWorld\nåäö")));
        assert_eq!(input.cursor_column(), 3);
        assert_eq!(input.cursor_row(), 2);

        let input = input.apply(InputOperation::Up).apply(InputOperation::Up);
        assert_eq!(input.cursor_column(), 3);
        assert_eq!(input.cursor_row(), 0);

        let input = input
            .apply(InputOperation::Right)
            .apply(InputOperation::Right);
        assert_eq!(input.cursor_column(), 5);
        assert_eq!(input.cursor_row(), 0);

        let input = input
            .apply(InputOperation::Down)
            .apply(InputOperation::Down);
        assert_eq!(input.cursor_column(), 3);
        assert_eq!(input.cursor_row(), 2);

        let input = input
            .apply(InputOperation::Down)
            .apply(InputOperation::Down);
        assert_eq!(input.cursor_column(), 3);
        assert_eq!(input.cursor_row(), 2);

        let input = input.apply(InputOperation::Up);
        assert_eq!(input.cursor_column(), 5);
        assert_eq!(input.cursor_row(), 1);

        let input = input
            .apply(InputOperation::Left)
            .apply(InputOperation::Left)
            .apply(InputOperation::Left)
            .apply(InputOperation::Left)
            .apply(InputOperation::Left)
            .apply(InputOperation::Down);
        assert_eq!(input.cursor_column(), 0);
        assert_eq!(input.cursor_row(), 2);

        // Applying down movement on last line should place cursor at the end of the line.
        let input = input.apply(InputOperation::Down);
        assert_eq!(input.cursor_column(), 3);
        assert_eq!(input.cursor_row(), 2);
    }

    #[test]
    fn test_completions() {
        let input = TextInput::new()
            .apply(InputOperation::Insert(String::from("Start with a [[.")))
            .apply(InputOperation::Left);
        assert_eq!(*input.completions(), None);

        let completions = vec![
            Completion::note_link("Start with a ".len().."Start with a [[".len(), "note"),
            Completion::note_link("Start with a ".len().."Start with a [[".len(), "link"),
            Completion::note_link("Start with a ".len().."Start with a [[".len(), "list"),
        ];
        let input = input.provide_completions(completions.clone());
        assert_eq!(
            *input.completions(),
            NonEmptySelection::new(completions.clone())
        );

        let input = input
            .apply(InputOperation::NextCompletion)
            .apply(InputOperation::NextCompletion)
            // Providing identical completions should not disrupt the selection.
            .provide_completions(completions)
            .apply(InputOperation::PreviousCompletion)
            .apply(InputOperation::Complete);

        assert_eq!(input.text(), String::from("Start with a [[link]]."));
        assert_eq!(input.cursor_pos(), "Start with a [[link]]".len())
    }

    #[test]
    fn test_empties_completion() {
        let input = TextInput::from("Some text")
            .provide_completions(vec![Completion::note_link(0..1, "Note")]);

        for operation in [
            InputOperation::Left,
            InputOperation::Right,
            InputOperation::Up,
            InputOperation::Down,
            InputOperation::Insert(String::from("test")),
            InputOperation::Backspace,
        ] {
            assert_eq!(input.clone().apply(operation).completions, None);
        }
    }
}
