//! Pushdown Automaton (PDA) data structures and simulator.
//!
//! The [`Pda`] struct holds an NFA-with-stack.  The
//! [`simulator::PdaSimulator`] supports both acceptance modes:
//! - **Final state** — all input is consumed and the current state is a final
//!   state.
//! - **Empty stack** — all input is consumed and the stack is empty.

use crate::automaton::{Automaton, StateId};

pub mod simulator;
pub mod stepper;

/// A single PDA transition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PdaTransition {
    /// Source state.
    pub from: StateId,
    /// Target state.
    pub to: StateId,
    /// Input symbol(s) to consume.  Empty string means no input consumed (λ).
    pub input_to_read: String,
    /// Stack string to pop.  The *first* character is the top of the stack.
    /// Empty string means no pop.
    pub string_to_pop: String,
    /// Stack string to push.  The *first* character becomes the new top.
    /// Empty string means no push.
    pub string_to_push: String,
}

/// A Pushdown Automaton.
#[derive(Debug, Clone, Default)]
pub struct Pda {
    /// Core automaton.
    pub automaton: Automaton,
    /// All transitions.
    pub(crate) transitions: Vec<PdaTransition>,
}

impl Pda {
    /// Creates an empty PDA.
    pub fn new() -> Self {
        Pda::default()
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
    pub fn add_transition(&mut self, t: PdaTransition) {
        self.transitions.push(t);
    }

    /// Returns all transitions leaving state `id`.
    pub fn transitions_from(&self, id: StateId) -> impl Iterator<Item = &PdaTransition> {
        self.transitions.iter().filter(move |t| t.from == id)
    }
}
