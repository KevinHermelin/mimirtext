use unicode_segmentation::GraphemeCursor;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug, PartialEq)]
pub struct TextInput {
    current: String,
    cursor_pos: usize,
    desired_column: usize,
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
        }
    }
    #[cfg(test)]
    pub fn new_with(text: &str, cursor_pos: usize) -> Self {
        Self {
            current: text.to_owned(),
            cursor_pos,
            desired_column: 0,
        }
    }
    fn cursor(&self) -> GraphemeCursor {
        GraphemeCursor::new(self.cursor_pos, self.current.len(), true)
    }
    pub fn text(&self) -> String {
        self.current.clone()
    }
    #[cfg(test)]
    fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }
    pub fn cursor_row(&self) -> usize {
        self.current[..self.cursor_pos].split('\n').count() - 1
    }
    pub fn cursor_column(&self) -> usize {
        self.current[..self.cursor_pos]
            .split('\n')
            .last()
            .expect("should have at least one line")
            .width()
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
                }
            }
            InputOperation::None => {}
        };
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
        assert_eq!(input.cursor_row(), 2);
        assert_eq!(input.cursor_column(), 3);

        let input = input
            .apply(InputOperation::Down)
            .apply(InputOperation::Down);
        assert_eq!(input.cursor_column(), 3);
        assert_eq!(input.cursor_row(), 2);

        let input = input.apply(InputOperation::Up);
        assert_eq!(input.cursor_column(), 5);
        assert_eq!(input.cursor_row(), 1);
    }
}
