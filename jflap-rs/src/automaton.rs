//! Core automaton data structures shared by FSA, PDA, and Turing machines.
//!
//! Each automaton is stored as an *arena*: states and transitions are owned by
//! the automaton and are referenced by lightweight integer IDs ([`StateId`]).
//! This avoids the circular-reference problem that would arise if states held
//! back-references to their owning automaton (as the original Java code does).

use std::collections::{HashMap, HashSet};

/// Opaque identifier for a state inside an [`Automaton`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StateId(pub u32);

/// A single state in an automaton.
#[derive(Debug, Clone)]
pub struct State {
    /// Numeric identifier (must be unique within the automaton).
    pub id: StateId,
    /// Human-readable name, e.g. `"q0"`.
    pub name: String,
    /// Optional descriptive label (shown in the GUI but not used by simulators).
    pub label: Option<String>,
    /// X/Y position for rendering (optional; stored for round-trip fidelity
    /// when reading/writing `.jff` files).
    pub x: f64,
    pub y: f64,
}

impl State {
    /// Creates a new state with the given numeric ID and name.
    pub fn new(id: StateId, name: impl Into<String>) -> Self {
        State {
            id,
            name: name.into(),
            label: None,
            x: 0.0,
            y: 0.0,
        }
    }
}

/// A low-level automaton that stores states and tracks initial/final-state sets.
///
/// This struct is *not* used directly by application code; the type-specific
/// wrappers ([`Fsa`][crate::fsa::Fsa], [`Pda`][crate::pda::Pda],
/// [`Tm`][crate::turing::Tm]) embed or delegate to this struct.
#[derive(Debug, Clone, Default)]
pub struct Automaton {
    /// All states, keyed by ID.
    pub(crate) states: HashMap<StateId, State>,
    /// Monotonically-increasing counter for generating fresh state IDs.
    next_id: u32,
    /// The designated initial state (if any).
    pub(crate) initial_state: Option<StateId>,
    /// Set of final (accepting) states.
    pub(crate) final_states: HashSet<StateId>,
}

impl Automaton {
    /// Creates an empty automaton.
    pub fn new() -> Self {
        Automaton::default()
    }

    /// Adds a new state with the given name and returns its ID.
    pub fn add_state(&mut self, name: impl Into<String>) -> StateId {
        let id = StateId(self.next_id);
        self.next_id += 1;
        self.states.insert(id, State::new(id, name));
        id
    }

    /// Adds a pre-built [`State`] (used when deserialising from XML where the
    /// ID and position are known ahead of time).
    pub fn add_state_with_id(&mut self, state: State) {
        let raw = state.id.0;
        if raw >= self.next_id {
            self.next_id = raw + 1;
        }
        self.states.insert(state.id, state);
    }

    /// Returns a reference to a state by ID.
    pub fn get_state(&self, id: StateId) -> Option<&State> {
        self.states.get(&id)
    }

    /// Returns an iterator over all states.
    pub fn states(&self) -> impl Iterator<Item = &State> {
        self.states.values()
    }

    /// Sets the initial state.
    pub fn set_initial_state(&mut self, id: StateId) {
        self.initial_state = Some(id);
    }

    /// Returns the initial state ID, if one has been set.
    pub fn initial_state(&self) -> Option<StateId> {
        self.initial_state
    }

    /// Marks a state as a final (accepting) state.
    pub fn add_final_state(&mut self, id: StateId) {
        self.final_states.insert(id);
    }

    /// Returns `true` if the state with the given ID is a final state.
    pub fn is_final_state(&self, id: StateId) -> bool {
        self.final_states.contains(&id)
    }

    /// Returns the set of final state IDs.
    pub fn final_states(&self) -> &HashSet<StateId> {
        &self.final_states
    }
}

/// Outcome returned by step-by-step simulators.
///
/// Returned by the `step()` method of [`crate::fsa::stepper::FsaStepper`],
/// [`crate::pda::stepper::PdaStepper`], and
/// [`crate::turing::stepper::TmStepper`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// The simulation is still running; call `step()` again for the next
    /// configuration.
    Active,
    /// The input has been **accepted**.
    Accepted,
    /// The input has been **rejected** (no further configurations remain).
    Rejected,
}
