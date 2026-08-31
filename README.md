# Mimir Text

![](docs/demo.gif)

Mimir Text is a lightweight tool to view and edit your Markdown wiki directly from the terminal.

A Markdown wiki is a collection of Markdown notes connected to each other through wikilinks. This creates a web of notes ordered more organically compared to traditional linear systems. This can be used to create documentation, a personal encyclopedia, or for a digital [Zettelkasten](https://en.wikipedia.org/wiki/Zettelkasten).

A wide variety of software exists for this purpose. Mimir Text is a lighter alternative that can be run directly in the terminal. Written in Rust, it is fast and has a tiny memory footprint.

## Features

- Browse your Markdown notes in the terminal
- Follow wikilinks between notes
- Support for rendering headings, lists, **bold**/*italic* text
- Built-in editor with auto-completion support when inserting links

## Building

Currently, the only way to run Mimir Text is by building it locally. With Nix installed, this is easily done through:

```sh
nix run github:KevinHermelin/mimirtext
```

Alternatively, you may build it directly using `cargo run` or `cargo build`. You will need the [Rust toolchain](https://rust-lang.org/) installed.


## Usage

Notes are stored as normal `.md` files in a directory, referred to as a repository. Any directory can be used as a repository. To create a directory "Notes" and open it as a repository in Mimir Text:

```sh
mkdir Notes
mimir Notes/Index.md
```

This will open an empty note `Index.md` in a newly created directory `Notes`. 

**Please note that Mimir Text will auto-save any changes made in the app directly to the files in the directory. This means that running the app inside a non-empty directory could potentially lead to unintended changes made to files in the directory. Back up your data and always run Mimir Text in a separate directory.**

There are two modes in Mimir Text. The mode is indicated by the mode display in the bottom-right corner.

Markdown notes (empty or not) are always opened in **browse mode**. From here, you can enter **edit mode** by pressing `C` to add content to your notes. Pressing `ESCAPE` writes any changes to file and changes the mode back to **browse mode**.

### Browse mode

In this mode, Markdown is rendered using styled text and you can navigate to other notes by following wikilinks. 

*If the mode display shows "SOURCE", you have opened a file that is not a Markdown (`.md`) note. Source mode works exactly as browse mode except that all styling and link selection are disabled.*

- Scroll using `UP` and `DOWN` arrow keys.
- Select a link using the `LEFT` and `RIGHT` arrow keys and follow it by pressing `ENTER`. 
    - Press `BACKSPACE` to navigate backwards. 
- Exit the app using `CTRL+Q`.
- Open the search window using `CTRL+P`.
    - The search window allows searching for notes by name.
    - Select a note in the list using `UP` and `DOWN` arrow keys and press `ENTER` to jump to the selected note.
    - Press `ESCAPE` to exit the search window.
- Press `C` to open the note in edit mode.

### Edit mode

In edit mode, the raw Markdown is opened for editing. Press `ESCAPE` to save your changes and exit edit mode. 

Notes ending in `.md` can be formatted using [Markdown](https://www.markdownguide.org). However, not all Markdown syntax is currently supported. 

Mimir Text also supports wikilinks. A wikilink is a shorter syntax for creating links to other notes in the same repository. As an example, `[[Note]]` creates a link with text `Note` pointing to a note with filename `Note.md`. The link text can be changed by adding an alias. `[[Note|Another name]]` creates a link to the same note but displays `Another name` instead. Links can be added both to existing and yet-to-be-created notes. The easiest way to create new notes is by adding links to them in pre-existing notes.

## License

Copyright 2026 Kevin Hermelin

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this package except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
