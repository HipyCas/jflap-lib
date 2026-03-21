//! Step-by-step FSA simulator for interactive UI integration.
//!
//! [`FsaStepper`] exposes a fine-grained stepping API designed for driving a
//! Tauri (or similar) UI: each call to [`FsaStepper::step`] advances by one
//! transition and returns a [`StepOutcome`].  The caller inspects
//! [`FsaStepper::configurations`] to obtain all currently-active NFA branches
//! for display.
//!
//! # Example
//!
//! ```rust
//! use jflap_rs::fsa::{Fsa, FsaTransition};
//! use jflap_rs::fsa::stepper::FsaStepper;
//! use jflap_rs::automaton::StepOutcome;
//!
//! let mut fsa = Fsa::new();
//! let s0 = fsa.add_state("q0");
//! let s1 = fsa.add_state("q1");
//! fsa.set_initial_state(s0);
//! fsa.add_final_state(s1);
//! fsa.add_transition(FsaTransition { from: s0, to: s0, label: "a".into() });
//! fsa.add_transition(FsaTransition { from: s0, to: s1, label: "b".into() });
//!
//! let mut stepper = FsaStepper::new(&fsa, "ab");
//! assert_eq!(stepper.step(), StepOutcome::Active);   // consumed "a"
//! assert_eq!(stepper.step(), StepOutcome::Accepted); // consumed "ab"
//! ```

use std::collections::HashSet;

use crate::automaton::{StateId, StepOutcome};
use crate::fsa::simulator::match_label;
use crate::fsa::Fsa;

/// A snapshot of one NFA branch at the current step, suitable for UI display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsaConfigSnapshot {
    /// ID of the active state.
    pub state_id: StateId,
    /// Human-readable name of the active state.
    pub state_name: String,
    /// Portion of the input already consumed.
    pub consumed: String,
    /// Portion of the input not yet consumed.
    pub remaining: String,
    /// Whether this state is a final (accepting) state.
    pub is_final: bool,
}

/// An interactive, step-by-step NFA simulator for [`Fsa`].
///
/// Each call to [`step`][FsaStepper::step] advances **all** currently-active
/// NFA branches by one transition (including lambda / ε transitions).
/// History is tracked so that [`step_back`][FsaStepper::step_back] can undo
/// the last step.
pub struct FsaStepper<'a> {
    fsa: &'a Fsa,
    input: String,
    /// `history[0]` = initial configurations.  `history[n]` = NEW configs
    /// added at step n (deduplicated via `visited`).
    history: Vec<Vec<(StateId, usize)>>,
    /// Every configuration ever active.  Restored incrementally on undo.
    visited: HashSet<(StateId, usize)>,
    /// True once the simulation has terminated.
    done: bool,
    /// True if any configuration has reached an accepting state with all input consumed.
    accepted: bool,
}

impl<'a> FsaStepper<'a> {
    /// Creates a new stepper for `fsa` on the given `input` string.
    pub fn new(fsa: &'a Fsa, input: &str) -> Self {
        let mut visited = HashSet::new();
        let initial: Vec<(StateId, usize)> = if let Some(s) = fsa.automaton.initial_state() {
            visited.insert((s, 0usize));
            vec![(s, 0usize)]
        } else {
            vec![]
        };
        let input_str = input.to_owned();
        let accepted = initial
            .iter()
            .any(|&(s, p)| p == input_str.len() && fsa.automaton.is_final_state(s));
        let done = initial.is_empty();
        FsaStepper {
            fsa,
            input: input_str,
            history: vec![initial],
            visited,
            done,
            accepted,
        }
    }

    /// Returns snapshots of all currently-active NFA branches.
    ///
    /// Results are sorted by state ID for deterministic ordering.
    pub fn configurations(&self) -> Vec<FsaConfigSnapshot> {
        let bytes = self.input.as_bytes();
        let mut snaps: Vec<FsaConfigSnapshot> = self
            .history
            .last()
            .into_iter()
            .flat_map(|v| v.iter())
            .map(|&(state_id, pos)| {
                let state_name = self
                    .fsa
                    .automaton
                    .get_state(state_id)
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| format!("q{}", state_id.0));
                FsaConfigSnapshot {
                    state_id,
                    state_name,
                    consumed: String::from_utf8_lossy(&bytes[..pos]).into_owned(),
                    remaining: String::from_utf8_lossy(&bytes[pos..]).into_owned(),
                    is_final: self.fsa.automaton.is_final_state(state_id),
                }
            })
            .collect();
        snaps.sort_by_key(|s| s.state_id);
        snaps
    }

    /// Returns the current step depth (0 = initial state, before any transitions).
    pub fn current_step(&self) -> usize {
        self.history.len().saturating_sub(1)
    }

    /// Advances the simulation by one transition step.
    ///
    /// Follows all applicable transitions (labelled **and** lambda) from each
    /// currently-active configuration.  Newly-reachable configurations
    /// (never seen before) form the next frontier.
    ///
    /// Returns:
    /// - [`StepOutcome::Active`] — more steps may follow.
    /// - [`StepOutcome::Accepted`] — at least one branch has consumed all input
    ///   in a final state.
    /// - [`StepOutcome::Rejected`] — no new configurations were reachable.
    pub fn step(&mut self) -> StepOutcome {
        if self.done {
            return self.outcome();
        }

        let current: Vec<(StateId, usize)> = match self.history.last() {
            Some(c) if !c.is_empty() => c.clone(),
            _ => {
                self.done = true;
                return StepOutcome::Rejected;
            }
        };

        let bytes = self.input.as_bytes();
        let mut next: Vec<(StateId, usize)> = Vec::new();

        for (state, pos) in current {
            for t in self.fsa.transitions_from(state) {
                let new_pos = if t.label.is_empty() {
                    // Lambda / ε transition — consume no input.
                    pos
                } else if let Some(len) = match_label(&t.label, bytes, pos) {
                    pos + len
                } else {
                    continue;
                };

                let config = (t.to, new_pos);
                if self.visited.insert(config) {
                    next.push(config);
                }
            }
        }

        if next.is_empty() {
            self.done = true;
            return if self.accepted {
                StepOutcome::Accepted
            } else {
                StepOutcome::Rejected
            };
        }

        let input_len = self.input.len();
        if next
            .iter()
            .any(|&(s, p)| p == input_len && self.fsa.automaton.is_final_state(s))
        {
            self.accepted = true;
        }
        self.history.push(next);

        if self.accepted {
            self.done = true;
            StepOutcome::Accepted
        } else {
            StepOutcome::Active
        }
    }

    /// Reverts to the previous step, properly restoring the visited set so
    /// that stepping forward again produces the same configurations.
    ///
    /// Returns `false` if already at the initial step (step 0).
    pub fn step_back(&mut self) -> bool {
        if self.history.len() <= 1 {
            return false;
        }
        // Remove the configs introduced at the last step from `visited`.
        let removed = self.history.pop().unwrap();
        for config in removed {
            self.visited.remove(&config);
        }
        self.done = false;
        // Recompute `accepted` for the restored frontier.
        let input_len = self.input.len();
        self.accepted = self
            .history
            .last()
            .into_iter()
            .flat_map(|v| v.iter())
            .any(|&(s, p)| p == input_len && self.fsa.automaton.is_final_state(s));
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
    ///
    /// Returns the final [`StepOutcome`].  If the simulation does not
    /// terminate within `max_steps`, returns [`StepOutcome::Rejected`].
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
    pub fn reset(&mut self) {
        let input = self.input.clone();
        *self = FsaStepper::new(self.fsa, &input);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsa::{Fsa, FsaTransition};

    fn build_astar_b() -> Fsa {
        let mut fsa = Fsa::new();
        let s0 = fsa.add_state("q0");
        let s1 = fsa.add_state("q1");
        fsa.set_initial_state(s0);
        fsa.add_final_state(s1);
        fsa.add_transition(FsaTransition {
            from: s0,
            to: s0,
            label: "a".into(),
        });
        fsa.add_transition(FsaTransition {
            from: s0,
            to: s1,
            label: "b".into(),
        });
        fsa
    }

    #[test]
    fn step_through_ab() {
        let fsa = build_astar_b();
        let mut stepper = FsaStepper::new(&fsa, "ab");

        assert_eq!(stepper.outcome(), StepOutcome::Active);
        // Step 1: consume "a" → stay in q0
        assert_eq!(stepper.step(), StepOutcome::Active);
        let cfgs = stepper.configurations();
        assert_eq!(cfgs.len(), 1);
        assert_eq!(cfgs[0].consumed, "a");
        assert_eq!(cfgs[0].remaining, "b");

        // Step 2: consume "b" → reach q1 (final) → accepted
        assert_eq!(stepper.step(), StepOutcome::Accepted);
        assert!(stepper.is_accepted());
    }

    #[test]
    fn step_rejects_ba() {
        let fsa = build_astar_b();
        let mut stepper = FsaStepper::new(&fsa, "ba");
        // From q0, "b" → q1; "a" from q1 → nothing → rejected
        let o1 = stepper.step();
        assert_eq!(o1, StepOutcome::Active);
        let o2 = stepper.step();
        assert_eq!(o2, StepOutcome::Rejected);
    }

    #[test]
    fn step_back_restores_state() {
        let fsa = build_astar_b();
        let mut stepper = FsaStepper::new(&fsa, "ab");

        stepper.step(); // step 1
        assert_eq!(stepper.current_step(), 1);

        let ok = stepper.step_back();
        assert!(ok);
        assert_eq!(stepper.current_step(), 0);

        // Stepping forward again should reproduce step 1.
        assert_eq!(stepper.step(), StepOutcome::Active);
        let cfgs = stepper.configurations();
        assert_eq!(cfgs[0].consumed, "a");
    }

    #[test]
    fn run_to_completion_accepts() {
        let fsa = build_astar_b();
        let mut stepper = FsaStepper::new(&fsa, "aab");
        assert_eq!(stepper.run_to_completion(100), StepOutcome::Accepted);
    }

    #[test]
    fn run_to_completion_rejects() {
        let fsa = build_astar_b();
        let mut stepper = FsaStepper::new(&fsa, "ba");
        assert_eq!(stepper.run_to_completion(100), StepOutcome::Rejected);
    }

    #[test]
    fn lambda_stepper() {
        // λ from q0 → q1 (final), labelled "a" from q0 → q2 (final)
        let mut fsa = Fsa::new();
        let s0 = fsa.add_state("q0");
        let s1 = fsa.add_state("q1");
        fsa.set_initial_state(s0);
        fsa.add_final_state(s1);
        fsa.add_transition(FsaTransition {
            from: s0,
            to: s1,
            label: "".into(),
        });

        // The initial state already has an accepting branch via lambda, but
        // it hasn't been stepped yet. After one step (following the lambda),
        // we should be accepted.
        let mut stepper = FsaStepper::new(&fsa, "");
        // Empty input → initial state is NOT final; need to follow lambda.
        assert_eq!(stepper.step(), StepOutcome::Accepted);
    }
}
