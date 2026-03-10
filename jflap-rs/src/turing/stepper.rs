//! Step-by-step TM simulator for interactive UI integration.
//!
//! [`TmStepper`] exposes a fine-grained stepping API designed for Tauri UIs:
//! each call to [`TmStepper::step`] executes exactly **one** TM transition and
//! returns a [`StepOutcome`].  The full tape state and head position are exposed
//! at every step via [`TmStepper::configuration`].
//!
//! Because a TM is deterministic, there is at most one active configuration at
//! any point.  Step history is stored as snapshots so that
//! [`step_back`][TmStepper::step_back] can faithfully undo a step.
//!
//! # Example
//!
//! ```rust
//! use jflap_rs::turing::{Tm, TmTransition, Direction};
//! use jflap_rs::turing::stepper::{TmStepper, AcceptMode};
//! use jflap_rs::automaton::StepOutcome;
//!
//! // Trivial TM: accept empty input immediately (initial state is also final).
//! let mut tm = Tm::new();
//! let q0 = tm.add_state("q0");
//! tm.set_initial_state(q0);
//! tm.add_final_state(q0);
//!
//! let mut stepper = TmStepper::new(&tm, "", AcceptMode::FinalState);
//! // Initial state is final → already accepted on creation.
//! assert_eq!(stepper.outcome(), StepOutcome::Accepted);
//! ```

use crate::automaton::{StateId, StepOutcome};
use crate::turing::tape::Tape;
use crate::turing::{Direction, Tm, TmTransition};

/// Re-export acceptance mode so callers need only import from this module.
pub use crate::turing::simulator::AcceptMode;

/// A snapshot of the TM at one step, suitable for UI display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmConfigSnapshot {
    /// ID of the current state.
    pub state_id: StateId,
    /// Human-readable name of the current state.
    pub state_name: String,
    /// Full tape contents as a string.
    pub tape: String,
    /// Logical head position (may be negative).
    pub head_pos: i64,
    /// Whether this state is a final (accepting) state.
    pub is_final: bool,
    /// Whether the machine has halted (no applicable transition).
    pub is_halted: bool,
    /// How many transitions have been taken so far.
    pub step_count: usize,
}

/// Internal snapshot stored in history.
#[derive(Clone)]
struct TmSnapshot {
    state_id: StateId,
    tape: Tape,
    halted: bool,
    step_count: usize,
}

/// An interactive, step-by-step TM simulator.
///
/// Since the TM is deterministic, there is always at most one configuration.
/// Call [`step`][TmStepper::step] to execute the next transition, and
/// [`configuration`][TmStepper::configuration] to inspect the current state.
pub struct TmStepper<'a> {
    tm: &'a Tm,
    accept_mode: AcceptMode,
    /// Full history of snapshots; `history[0]` = initial state.
    history: Vec<TmSnapshot>,
    done: bool,
    accepted: bool,
}

impl<'a> TmStepper<'a> {
    /// Creates a new stepper for `tm` on the given `input` string.
    pub fn new(tm: &'a Tm, input: &str, accept_mode: AcceptMode) -> Self {
        let initial_state = tm.automaton.initial_state();
        let tape = Tape::new(input);
        let (done, accepted) = if let Some(s) = initial_state {
            let accepted = accept_mode == AcceptMode::FinalState && tm.automaton.is_final_state(s);
            (accepted, accepted)
        } else {
            (true, false)
        };
        let initial_snap = TmSnapshot {
            state_id: initial_state.unwrap_or(StateId(0)),
            tape,
            halted: false,
            step_count: 0,
        };
        TmStepper {
            tm,
            accept_mode,
            history: vec![initial_snap],
            done,
            accepted,
        }
    }

    /// Returns a snapshot of the current TM configuration.
    pub fn configuration(&self) -> TmConfigSnapshot {
        let snap = self.history.last().unwrap();
        let state_id = snap.state_id;
        let state_name = self
            .tm
            .automaton
            .get_state(state_id)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| format!("q{}", state_id.0));
        TmConfigSnapshot {
            state_id,
            state_name,
            tape: snap.tape.contents(),
            head_pos: snap.tape.head_pos(),
            is_final: self.tm.automaton.is_final_state(state_id),
            is_halted: snap.halted,
            step_count: snap.step_count,
        }
    }

    /// Returns the current step count (0 = initial state).
    pub fn current_step(&self) -> usize {
        self.history.last().map(|s| s.step_count).unwrap_or(0)
    }

    /// Executes one TM transition.
    ///
    /// Returns:
    /// - [`StepOutcome::Active`] — transition taken; more steps may follow.
    /// - [`StepOutcome::Accepted`] — the machine has accepted the input.
    /// - [`StepOutcome::Rejected`] — the machine halted in a non-final state.
    pub fn step(&mut self) -> StepOutcome {
        if self.done {
            return self.outcome();
        }

        let current = self.history.last().unwrap().clone();
        let state = current.state_id;
        let symbol = current.tape.read();

        // In FinalState mode: check acceptance before attempting any transition.
        if self.accept_mode == AcceptMode::FinalState && self.tm.automaton.is_final_state(state) {
            self.done = true;
            self.accepted = true;
            return StepOutcome::Accepted;
        }

        match self.find_transition(state, symbol) {
            None => {
                // Machine halts — no transition taken, step_count stays the same.
                self.done = true;
                self.accepted = self.tm.automaton.is_final_state(state);
                // Record halted snapshot (step_count is NOT incremented — no
                // transition was executed).
                let mut snap = current.clone();
                snap.halted = true;
                self.history.push(snap);
                if self.accepted {
                    StepOutcome::Accepted
                } else {
                    StepOutcome::Rejected
                }
            }
            Some(t) => {
                let mut new_tape = current.tape.clone();
                let to_write = if t.write == '~' { symbol } else { t.write };
                new_tape.write(to_write);
                match t.direction {
                    Direction::Left => new_tape.move_left(),
                    Direction::Right => new_tape.move_right(),
                    Direction::Stay => {}
                }

                let new_state = t.to;
                // Check acceptance immediately after transition in FinalState mode.
                if self.accept_mode == AcceptMode::FinalState
                    && self.tm.automaton.is_final_state(new_state)
                {
                    self.done = true;
                    self.accepted = true;
                    self.history.push(TmSnapshot {
                        state_id: new_state,
                        tape: new_tape,
                        halted: false,
                        step_count: current.step_count + 1,
                    });
                    return StepOutcome::Accepted;
                }

                self.history.push(TmSnapshot {
                    state_id: new_state,
                    tape: new_tape,
                    halted: false,
                    step_count: current.step_count + 1,
                });
                StepOutcome::Active
            }
        }
    }

    /// Reverts to the previous step.
    ///
    /// Returns `false` if already at the initial step.
    pub fn step_back(&mut self) -> bool {
        if self.history.len() <= 1 {
            return false;
        }
        self.history.pop();
        self.done = false;
        self.accepted = false;
        // Recheck: was the restored state already accepted in FinalState mode?
        if let Some(snap) = self.history.last() {
            if self.accept_mode == AcceptMode::FinalState
                && self.tm.automaton.is_final_state(snap.state_id)
            {
                self.done = true;
                self.accepted = true;
            }
        }
        true
    }

    /// Returns `true` if the simulation has accepted the input.
    pub fn is_accepted(&self) -> bool {
        self.accepted
    }

    /// Returns the current outcome without advancing the simulation.
    pub fn outcome(&self) -> StepOutcome {
        if self.accepted {
            StepOutcome::Accepted
        } else if self.done {
            StepOutcome::Rejected
        } else {
            StepOutcome::Active
        }
    }

    /// Runs the simulation to completion (up to `max_steps` transitions).
    pub fn run_to_completion(&mut self, max_steps: usize) -> StepOutcome {
        for _ in 0..max_steps {
            let o = self.step();
            if o != StepOutcome::Active {
                return o;
            }
        }
        StepOutcome::Rejected
    }

    /// Resets the stepper to the initial configuration.
    pub fn reset(&mut self, input: &str) {
        *self = TmStepper::new(self.tm, input, self.accept_mode);
    }

    fn find_transition(&self, state: StateId, symbol: char) -> Option<&TmTransition> {
        let mut wildcard: Option<&TmTransition> = None;

        for t in self.tm.transitions_from(state) {
            if t.read == symbol {
                return Some(t); // exact match wins immediately
            } else if t.read == '~' && wildcard.is_none() {
                wildcard = Some(t); // wildcard is the only valid fallback
            }
        }
        wildcard
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::turing::tape::BLANK;
    use crate::turing::{Direction, Tm, TmTransition};

    /// Build a simple TM: replace every 'a' with 'b', halt when blank reached.
    /// The halting state (q1) is final → accepts all inputs.
    fn build_replace_tm() -> Tm {
        let mut tm = Tm::new();
        let q0 = tm.add_state("q0");
        let q1 = tm.add_state("q1");
        tm.set_initial_state(q0);
        tm.add_final_state(q1);
        // Replace 'a' with 'b', move right.
        tm.add_transition(TmTransition {
            from: q0,
            to: q0,
            read: 'a',
            write: 'b',
            direction: Direction::Right,
        });
        // Halt when blank — transition to final state.
        tm.add_transition(TmTransition {
            from: q0,
            to: q1,
            read: BLANK,
            write: BLANK,
            direction: Direction::Stay,
        });
        tm
    }

    /// Build a TM that accepts when it reaches a final state (FinalState mode):
    /// initial state is immediately a final state.
    fn build_immediate_accept_tm() -> Tm {
        let mut tm = Tm::new();
        let q0 = tm.add_state("q0");
        tm.set_initial_state(q0);
        tm.add_final_state(q0);
        // Add a transition so it doesn't halt immediately, to test FinalState
        // detection on creation.
        tm.add_transition(TmTransition {
            from: q0,
            to: q0,
            read: 'a',
            write: 'a',
            direction: Direction::Right,
        });
        tm
    }

    // ─── HaltInFinalState tests ─────────────────────────────────────────────

    #[test]
    fn halt_in_final_accepts_aaa() {
        let tm = build_replace_tm();
        let mut stepper = TmStepper::new(&tm, "aaa", AcceptMode::HaltInFinalState);
        assert_eq!(stepper.run_to_completion(100), StepOutcome::Accepted);
        // All a's replaced with b's.
        assert_eq!(stepper.configuration().tape.trim_matches(BLANK), "bbb");
    }

    #[test]
    fn halt_in_final_accepts_empty() {
        let tm = build_replace_tm();
        let mut stepper = TmStepper::new(&tm, "", AcceptMode::HaltInFinalState);
        assert_eq!(stepper.run_to_completion(10), StepOutcome::Accepted);
    }

    #[test]
    fn step_back_restores_tape() {
        let tm = build_replace_tm();
        let mut stepper = TmStepper::new(&tm, "a", AcceptMode::HaltInFinalState);
        stepper.step(); // replaces 'a' with 'b'
        let after_one = stepper.configuration().tape.clone();
        stepper.step_back();
        let initial_tape = stepper.configuration().tape.clone();
        // Initial tape should contain 'a', not 'b'.
        assert!(
            initial_tape.contains('a'),
            "tape should contain 'a' after undo"
        );
        assert!(
            !after_one.contains('a'),
            "tape after step should not contain 'a'"
        );
    }

    // ─── FinalState mode tests ──────────────────────────────────────────────

    #[test]
    fn final_state_mode_immediate_accept() {
        let tm = build_immediate_accept_tm();
        // q0 is both initial and final → accepted immediately.
        let stepper = TmStepper::new(&tm, "a", AcceptMode::FinalState);
        assert_eq!(stepper.outcome(), StepOutcome::Accepted);
        assert!(stepper.is_accepted());
    }

    #[test]
    fn final_state_mode_accept_after_transition() {
        // q0 (initial) --'a'/'X'/R--> q1 (final)
        let mut tm = Tm::new();
        let q0 = tm.add_state("q0");
        let q1 = tm.add_state("q1");
        tm.set_initial_state(q0);
        tm.add_final_state(q1);
        tm.add_transition(TmTransition {
            from: q0,
            to: q1,
            read: 'a',
            write: 'X',
            direction: Direction::Right,
        });

        let mut stepper = TmStepper::new(&tm, "a", AcceptMode::FinalState);
        // Not yet accepted (q0 is not final).
        assert_eq!(stepper.outcome(), StepOutcome::Active);
        // One step: write 'X', move right, enter q1 (final) → accepted.
        assert_eq!(stepper.step(), StepOutcome::Accepted);
    }

    #[test]
    fn final_state_mode_step_back() {
        let mut tm = Tm::new();
        let q0 = tm.add_state("q0");
        let q1 = tm.add_state("q1");
        tm.set_initial_state(q0);
        tm.add_final_state(q1);
        tm.add_transition(TmTransition {
            from: q0,
            to: q1,
            read: 'a',
            write: 'X',
            direction: Direction::Right,
        });

        let mut stepper = TmStepper::new(&tm, "a", AcceptMode::FinalState);
        assert_eq!(stepper.step(), StepOutcome::Accepted);
        assert!(stepper.step_back());
        // After undo, back to q0 which is not final.
        assert_eq!(stepper.outcome(), StepOutcome::Active);
        assert!(!stepper.is_accepted());
    }
}
