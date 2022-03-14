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

Possible future features:
- Auto-completion and hints
- Line history

The API should be considered experimental and will change in the
future.


The core consists of a massive state machine taking bytes as input
and returning an iterator over byte slices. There are, however,
some convenience wrappers:
- [`sync::Editor`]
  - [`sync::std::IO`]
  - [`sync::embedded::IO`]
- [`no_sync::tokio::Editor`]

## Example
```rust
use noline::sync::{std::IO, Editor};
use std::io;
use std::fmt::Write;
use termion::raw::IntoRawMode;

fn main() {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout().into_raw_mode().unwrap();
    let prompt = "> ";

    let mut io = IO::new(stdin, stdout);
    let mut editor = Editor::<Vec<u8>, _>::new(&mut io).unwrap();

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
noline = "0.1.0"
```

# License

MPL-2.0
