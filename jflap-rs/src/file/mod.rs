//! File I/O for JFLAP `.jff` XML files.
//!
//! The [`load`] function reads a `.jff` file from a path or an in-memory
//! string and returns a typed [`Automaton`] enum.

pub mod xml;

pub use xml::{load_file, load_str, AutomatonKind, ParseError};
