//! File I/O for JFLAP `.jff` XML files.
//!
//! Use [`load_file`] to read from disk, or [`load_str`] for an in-memory XML
//! string.  Both return an [`AutomatonKind`] enum that wraps the parsed
//! automaton.

pub mod xml;

pub use xml::{load_file, load_str, AutomatonKind, ParseError};
