# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Fix cursor positioning with multibyte prompts.
- Fix panic on missing CPR arguments.
- Reject malformed UTF-8 and recover interrupted input.
- Restore wrapped UTF-8 history safely; reject oversized entries.
- Preserve terminal output when I/O accepts partial writes.
- Avoid empty history entries at the circular buffer boundary.

## [0.5.1 - 2026-03-31]

- Update outdated packages
- Fix examples to build with updated packages
- Added a lifetime specifier to prevent a lint about elided lifetimes
- Fixed a bug in the sync rp2040 example where the flush impl wasn't polling, causing the cli to break entirely.

## [0.5.0 - 2024-12-12]

- Removed initializer. Probe terminal size before prompt for every line.

## [0.5.0 - 2024-08-30]

- Bugfix: Char boundary string splitting error
- Allow dynamic prompt made from iterator over `&str`
- Take slice instead of owning buffer for line and history

## [0.4.0 - 2024-08-27]

- Fixed no_std examples build failure by checking in lock files
- Removed IO abstraction
- Fixed clippy warnings
- Removed `sync` and `async` features
- Removed `std` from default features

## [0.3.0 - 2024-07-05]

- Use embedded_io and embedded_io_async Read/Write traits
  - This ensures the std and no-std implementations are equivilent at the IO API interface
- Add sync and async examples for rp2040 async makes use of [embassy](https://embassy.dev/)
- Removed stm32 example as there was no hardware available (can be replaced later)

## [0.2.1] - 2024-06-06

- Added Linefeed as a valid line terminator
- Fixed overflow error when attempting to navigate empty history

## [0.2.0] - 2022-03-22

- Added basic line history

- Added EditorBuilder for more ergonomic construction of editors

## [0.1.0] - 2022-03-14

- Initial release
