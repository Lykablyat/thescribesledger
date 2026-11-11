# The Scribe's Ledger

A distraction-free novel writing studio built in Rust, featuring a Scrivener-style binder, custom formatting, and print-ready PDF export via Typst.

## Features

- **Manuscript Binder**: Organize chapters and scenes with intuitive drag-and-drop reordering.
- **Focus Writing Canvas**: Clean, paper-style writing area with real-time word counting and zoom controls.
- **Rich Formatting**: Bold, italic, and underline tools with an optional toggle to hide raw Markdown markup.
- **Specialized Sections**: Support for Prologues, Epilogues, Forewords, and Author's Notes.
- **Print Compilation**: Export novel manuscripts directly to 6" x 9" book-formatted PDFs using Typst.
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

To compile standalone binaries:
```bash
cargo build --release
```
The executable will be located at `target/release/thescribesledger.exe`.

### Optional: Typst CLI

For direct manuscript PDF compilation from the UI, install [Typst](https://github.com/typst/typst):
```bash
winget install --id Typst.Typst
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
