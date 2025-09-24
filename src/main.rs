mod markdown;
mod model;
mod repository;
mod text_input;
mod tui;

use color_eyre::Result;

fn main() -> Result<()> {
    tui::main()?;
    Ok(())
}
