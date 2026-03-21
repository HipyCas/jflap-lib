//! Finite State Automaton (FSA / NFA) data structures and simulator.
//!
//! The [`Fsa`] struct stores an NFA (non-deterministic finite automaton),
//! including lambda (ε) transitions.  The [`simulator::FsaSimulator`] performs
//! BFS over the reachable configurations to decide acceptance.

use crate::automaton::{Automaton, StateId};

pub mod simulator;
pub mod stepper;

/// A single FSA transition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FsaTransition {
    /// The source state.
    pub from: StateId,
    /// The target state.
    pub to: StateId,
    /// The label to consume.  An empty string (`""`) represents a lambda / ε
    /// (epsilon) transition.
    pub label: String,
}

/// A Finite State Automaton (NFA with optional lambda transitions).
#[derive(Debug, Clone, Default)]
pub struct Fsa {
    /// Core automaton (states, initial/final sets).
    pub automaton: Automaton,
    /// All transitions.
    pub(crate) transitions: Vec<FsaTransition>,
}

impl Fsa {
    /// Creates an empty FSA.
    pub fn new() -> Self {
        Fsa::default()
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
    pub fn add_transition(&mut self, t: FsaTransition) {
        self.transitions.push(t);
    }

    /// Returns all transitions leaving state `id`.
    pub fn transitions_from(&self, id: StateId) -> impl Iterator<Item = &FsaTransition> {
        self.transitions.iter().filter(move |t| t.from == id)
    }
}
