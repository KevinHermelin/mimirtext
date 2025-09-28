use unicode_segmentation::GraphemeCursor;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug, PartialEq)]
pub struct TextInput {
    current: String,
    cursor_pos: usize,
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
        }
    }
    #[cfg(test)]
    pub fn new_with(text: &str, cursor_pos: usize) -> Self {
        Self {
            current: text.to_owned(),
            cursor_pos,
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
    pub fn cursor_column(&self) -> usize {
        self.current[..self.cursor_pos].width()
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
            }
            InputOperation::Backspace => {
                let current_pos = self.cursor().cur_cursor();
                if let Some(new_pos) = self.move_cursor(Movement::PreviousBoundary) {
                    self.cursor_pos = new_pos;
                    self.current.drain(new_pos..current_pos);
                }
            }
            InputOperation::Left => {
                if let Some(new_pos) = self.move_cursor(Movement::PreviousBoundary) {
                    self.cursor_pos = new_pos;
                }
            }
            InputOperation::Right => {
                if let Some(new_pos) = self.move_cursor(Movement::NextBoundary) {
                    self.cursor_pos = new_pos;
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
}
