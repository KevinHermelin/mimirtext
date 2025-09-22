use std::cmp::max;

use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::Stylize,
    text::Line,
    widgets::{Paragraph, Widget},
};

pub fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
    let [area] = Layout::horizontal([horizontal])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);
    area
}

pub struct NonIdealState {
    caption: String,
    subcaption: String,
}

impl NonIdealState {
    pub fn new(caption: &str, subcaption: &str) -> Self {
        Self {
            caption: caption.to_owned(),
            subcaption: subcaption.to_owned(),
        }
    }
}

impl Widget for &NonIdealState {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let caption = self.caption.clone().bold();
        let subcaption = self.subcaption.clone().dim();
        let min_width = max(caption.width(), subcaption.width()) as u16;

        let area = center(area, Constraint::Length(min_width), Constraint::Length(5));

        Paragraph::new(vec![
            Line::from(caption),
            Line::from(""),
            Line::from(subcaption),
        ])
        .alignment(Alignment::Center)
        .render(area, buf);
    }
}
