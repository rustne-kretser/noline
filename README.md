![Pipeline](https://github.com/rustne-kretser/noline/actions/workflows/rust.yml/badge.svg)
[![Crates.io](https://img.shields.io/crates/v/noline.svg)](https://crates.io/crates/noline)
[![API reference](https://docs.rs/noline/badge.svg)](https://docs.rs/noline/)

# noline

Noline is an IO-agnostic `#[no_std]` line editor providing robust
line editing for any system. The core functionality is IO-free, so
it can be adapted to any system be it embedded, async, async
embedded, WASM or IPoAC (IP over Avian Carriers).

Features:
- IO-free
- Minimal dependencies
- No allocation needed - Both heap-based and static buffers are provided
- UTF-8 support
- Emacs keybindings
- Line history

Possible future features:
- Auto-completion and hints

The API should be considered experimental and will change in the
future.

The core implementation consists of a state machine taking bytes as
input and yielding iterators over byte slices. Because this is
done without any IO, Noline can be adapted to work on any platform.

Noline comes with multiple implemenations:
- [`sync_editor::Editor`] – Editor for synchronous IO
- [`async_editor::Editor`] - Editor for asynchronous IO

Both offer `line(prompt)` for application-managed editing.

Editors can be built using [`builder::EditorBuilder`].

## Example
```rust
let prompt = "> ";

let mut io = MyIO {}; // IO handler, see full examples for details
                      // how to implement it

let mut buffer = [0; 128];
let mut history = [0; 512];
let mut editor = EditorBuilder::from_slice(&mut buffer)
    .with_slice_history(&mut history)
    .build_sync(&mut io)
    .unwrap();

while let Ok(line) = editor.readline(prompt, &mut io) {
    writeln!(io, "Read: '{}'", line).unwrap();
}
```

For more details, see [docs](https://docs.rs/noline/).

# Usage

Add this to your Cargo.toml:

```toml
[dependencies]
noline = "0.5.1"
```

# License

MPL-2.0
