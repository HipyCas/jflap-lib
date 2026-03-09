//! Turing Machine data structures and simulator.
//!
//! This module ports the single-tape variant of JFLAP's Turing machine.
//! The [`tape`] sub-module provides a bi-infinite tape backed by a
//! [`Vec`]-based buffer.

use std::str::FromStr;

use crate::automaton::{Automaton, StateId};

pub mod simulator;
pub mod tape;

/// The direction a TM tape head can move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Move head one cell to the left.
    Left,
    /// Move head one cell to the right.
    Right,
    /// Do not move the head (stay).
    Stay,
}

/// Error returned when parsing a direction string fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectionParseError(pub String);

impl std::fmt::Display for DirectionParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid direction string: {:?}", self.0)
    }
}

impl FromStr for Direction {
    type Err = DirectionParseError;

    /// Parses a JFLAP direction string (`"L"`, `"R"`, or `"S"`).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "L" => Ok(Direction::Left),
            "R" => Ok(Direction::Right),
            "S" => Ok(Direction::Stay),
            other => Err(DirectionParseError(other.to_owned())),
        }
    }
}

/// A single TM transition (single-tape).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TmTransition {
    /// Source state.
    pub from: StateId,
    /// Target state.
    pub to: StateId,
    /// Symbol to read from the tape. `'~'` is a wildcard that matches any
    /// symbol.
    pub read: char,
    /// Symbol to write to the tape. `'~'` means "write what was read" (no
    /// change).
    pub write: char,
    /// Direction to move the head.
    pub direction: Direction,
}

/// A single-tape Turing Machine.
#[derive(Debug, Clone, Default)]
pub struct Tm {
    /// Core automaton (states, initial/final).
    pub automaton: Automaton,
    /// All transitions.
    pub(crate) transitions: Vec<TmTransition>,
}

impl Tm {
    /// Creates an empty Turing machine.
    pub fn new() -> Self {
        Tm::default()
    }

    /// Delegates to [`Automaton::add_state`].
    pub fn add_state(&mut self, name: impl Into<String>) -> StateId {
        self.automaton.add_state(name)
    }

    /// Delegates to [`Automaton::set_initial_state`].
    pub fn set_initial_state(&mut self, id: StateId) {
        self.automaton.set_initial_state(id);
    }

    /// Delegates to [`Automaton::add_final_state`].
    pub fn add_final_state(&mut self, id: StateId) {
        self.automaton.add_final_state(id);
    }

    /// Adds a transition.
    pub fn add_transition(&mut self, t: TmTransition) {
        self.transitions.push(t);
    }

    /// Returns all transitions leaving state `id`.
    pub fn transitions_from(&self, id: StateId) -> impl Iterator<Item = &TmTransition> {
        self.transitions.iter().filter(move |t| t.from == id)
    }
}
