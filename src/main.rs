mod document;
mod graph;
mod model;
mod repository;
mod selection;
mod text_input;
mod tui;
mod upstream;

use color_eyre::Result;

fn main() -> Result<()> {
    tui::main()?;
    Ok(())
}
