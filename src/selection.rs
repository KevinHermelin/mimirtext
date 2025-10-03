/// A non-empty list of items with a guaranteed selection.
///
/// Guarantees:
/// - The list always contains at least one item.
/// - One of the items is always selected.
#[derive(Clone, Debug, PartialEq)]
pub struct NonEmptySelection<T> {
    items: Vec<T>,
    selected: usize,
}

impl<T> NonEmptySelection<T> {
    /// Creates a new selection among given `items` and selects the first.
    ///
    /// Returns None if `items` contains no elements.
    pub fn new(items: Vec<T>) -> Option<Self> {
        if items.len() == 0 {
            return None;
        }
        Some(Self { items, selected: 0 })
    }

    pub fn items(&self) -> &Vec<T> {
        &self.items
    }

    pub fn next(mut self) -> Self {
        self.selected = self
            .selected
            .saturating_add(1)
            .clamp(0, self.items.len() - 1);
        self
    }

    pub fn previous(mut self) -> Self {
        self.selected = self
            .selected
            .saturating_sub(1)
            .clamp(0, self.items.len() - 1);
        self
    }

    pub fn selected(&self) -> &T {
        self.items.get(self.selected).expect("should be an element")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        assert_eq!(NonEmptySelection::<&str>::new(vec![]), None);

        assert_eq!(
            NonEmptySelection::<&str>::new(vec!["Element1"]),
            Some(NonEmptySelection {
                items: vec!["Element1"],
                selected: 0
            })
        );

        assert_eq!(
            NonEmptySelection::<&str>::new(vec!["Element1, Element2"]),
            Some(NonEmptySelection {
                items: vec!["Element1, Element2"],
                selected: 0
            })
        );
    }

    #[test]
    fn test_items() {
        let selection = NonEmptySelection::new(vec!["OptionA", "OptionB", "OptionC"]).unwrap();

        assert_eq!(*selection.items(), vec!["OptionA", "OptionB", "OptionC"]);
    }

    #[test]
    fn test_selection() {
        let selection = NonEmptySelection::new(vec!["OptionA", "OptionB", "OptionC"]).unwrap();

        assert_eq!(*selection.selected(), "OptionA");

        let selection = selection.next();
        assert_eq!(*selection.selected(), "OptionB");

        let selection = selection.previous().previous();
        assert_eq!(*selection.selected(), "OptionA");

        let selection = selection.next().next();
        assert_eq!(*selection.selected(), "OptionC");

        let selection = selection.next();
        assert_eq!(*selection.selected(), "OptionC");
    }
}
