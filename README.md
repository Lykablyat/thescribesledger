# The Scribe's Ledger (v1.1.0)

A distraction-free novel writing studio built in Rust (`eframe` / `egui`), featuring a Scrivener-style binder, custom editor profiles, dark mode, extensible typography, in-manuscript search & replace, and print-ready PDF export via Typst.

## Features

- **Manuscript Binder**: Organize chapters and scenes with drag-and-drop reordering, writer-only scene/chapter titles, and quick Front Matter navigation.
- **Focus Writing Canvas**: Clean, paper-style writing area with real-time word counting, line-spacing presets, and zoom controls.
- **Rich Formatting**: Bold (`Ctrl+B`), italic (`Ctrl+I`), and underline (`Ctrl+U`) tools with an optional toggle to hide raw Markdown markup (`Markdown In Editor`).
- **Dark Mode & Custom Profiles**: Full dark UI theme alongside built-in and custom writing profiles (customizable page color, ink color, font family, font size, and line spacing) persisted in `editor_settings.toml`.
- **Expanded Typography & Custom `.ttf` Font Import**:
  - Wide library of classic serif, sans-serif, and monospace fonts (`Sitka Text`, `Georgia`, `Times New Roman`, `Garamond`, `Palatino Linotype`, `Book Antiqua`, `Century`, `Constantia`, `Helvetica`, `Arial`, `Calibri`, `Segoe UI`, `Verdana`, `Consolas`, `Courier New`).
  - Import singular `.ttf` font files directly into the available font list, integrate them into custom profiles, and use them in PDF compilation.
- **Find & Change Findings (`Ctrl+F` / `F3`)**:
  - Compact top-left pop-up over the writing panel with scope selection (`Find in Scene`, `Find in Chapter`, `Find in Manuscript`).
  - Highlighted match navigation, optional `Change Findings` replacement mode, and a safety confirmation modal before modifying multiple matches.
- **Customizable PDF Compilation (`Compile PDF`)**:
  - Clicking **Compile PDF** opens a compilation options menu with **Default Print** (6" × 9" book layout, `Libertinus Serif` / `Georgia`, `10.5 pt`, black ink on white paper) or **Custom Styling** (choose any built-in or imported `.ttf` font, custom font size, text color, and page background color, or match your active editor profile) with a live preview before exporting via Typst (or the built-in Rust PDF fallback).
- **Specialized Sections**: Support for Prologues, Epilogues, Forewords, Author's Notes, and custom unnumbered headings.
- **Scrivener Importer**: Import existing `.scriv` projects (folders and `.scrivx` files).

## Installation & Running

Ensure you have Rust installed (1.80 or newer).

```bash
# Clone the repository
git clone https://github.com/Lykablyat/thescribesledger.git
cd thescribesledger

# Run the application
cargo run --release
```

To compile a standalone release binary:

```bash
cargo build --release
```

The executable will be located at `target/release/the-scribes-ledger.exe` (and can be copied to `The Scribes Ledger.exe` in the project root).

### Optional: Typst CLI

For direct 6" × 9" book-formatted PDF compilation (including custom `.ttf` fonts and styling) from the UI, install [Typst](https://github.com/typst/typst):

```bash
winget install --id Typst.Typst
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
