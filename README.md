# Mimir

![](docs/demo.gif)

Mimir is a lightweight tool to view and edit your Markdown wiki directly from the terminal.

## Features

- Browse your Markdown notes in the terminal
- Follow wikilinks between notes
- Support for rendering headings, lists, **bold**/*italic* text
- Built-in editor with auto-completion support when inserting links

## Building

Currently, the only way to run Mimir is by building it locally. With Nix installed, this is easily done through:

```sh
nix run github:KevinHermelin/mimirnotes
```

Alternatively, you may build it directly using `cargo run` or `cargo build`. You will need the [Rust toolchain](https://rust-lang.org/) installed.


## Usage

Notes are stored as normal `.md` files in a directory, referred to as a repository. Any directory can be used as a repository. To create a directory "Notes" and open it as a repository in Mimir:

```sh
mkdir Notes
mimir Notes/Index.md
```

This will open an empty index note in a newly created directory "Notes".

**Please note that Mimir will auto-save any changes made in the app directly to the files in the directory. This means that running Mimir inside a non-empty directory could potentially lead to unintended changes made to files in the directory. Back up your data and always run Mimir in a separate directory.**

### Browse mode

Markdown notes (empty or not) are always opened in "Browse" mode. This is indicated by "BROWSE" by the mode display in the bottom-right corner.

*If the mode display shows "SOURCE", you have opened a file that is not a `.md` note.*

In this mode, Markdown is rendered and you can navigate using wikilinks. 
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
