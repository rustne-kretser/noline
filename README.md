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

The core implementation consists of a state machie taking bytes as
input and yielding iterators over byte slices. Because this is
done without any IO, Noline can be adapted to work on any platform.

Noline comes with multiple implemenations:
- [`sync::Editor`] – Editor for asynchronous IO with two separate IO wrappers:
  - [`sync::std::IO`] – IO wrapper for [`std::io::Read`] and [`std::io::Write`] traits
  - [`sync::embedded::IO`] – IO wrapper for [`embedded_hal::serial::Read`] and [`embedded_hal::serial::Write`]
- [`no_sync::tokio::Editor`] - Editor for [`tokio::io::AsyncRead`] and [`tokio::io::AsyncWrite`]

Editors can be built using [`builder::EditorBuilder`].

## Example
```rust
use noline::{sync::std::IO, builder::EditorBuilder};
use std::fmt::Write;
use std::io;
use termion::raw::IntoRawMode;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout().into_raw_mode().unwrap();
    let prompt = "> ";

    let mut io = IO::new(stdin, stdout);

    let mut editor = EditorBuilder::new_unbounded()
        .with_unbounded_history()
        .build_sync(&mut io)
        .unwrap();

    loop {
        if let Ok(line) = editor.readline(prompt, &mut io) {
            write!(io, "Read: '{}'\n\r", line).unwrap();
        } else {
            break;
        }
    }
}
```

For more details, see [docs](https://docs.rs/noline/).

# Usage

Add this to your Cargo.toml:

```toml
[dependencies]
noline = "0.2.0"
```

# License

MPL-2.0
