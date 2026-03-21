//! Step-by-step PDA simulator for interactive UI integration.
//!
//! [`PdaStepper`] works like [`crate::fsa::stepper::FsaStepper`] but tracks
//! the stack contents in each configuration.  Both final-state and empty-stack
//! acceptance modes are supported.
//!
//! # Example
//!
//! ```rust
//! use jflap_rs::pda::{Pda, PdaTransition};
//! use jflap_rs::pda::simulator::AcceptMode;
//! use jflap_rs::pda::stepper::PdaStepper;
//! use jflap_rs::automaton::StepOutcome;
//!
//! let mut pda = Pda::new();
//! let q0 = pda.add_state("q0");
//! let q1 = pda.add_state("q1");
//! pda.set_initial_state(q0);
//! pda.add_final_state(q1);
//! pda.add_transition(PdaTransition {
//!     from: q0, to: q1,
//!     input_to_read: "a".into(),
//!     string_to_pop: "Z".into(),
//!     string_to_push: "Z".into(),
//! });
//!
//! let mut stepper = PdaStepper::new(&pda, "a", AcceptMode::FinalState);
//! assert_eq!(stepper.step(), StepOutcome::Accepted);
//! ```

use std::collections::HashSet;

use crate::automaton::{StateId, StepOutcome};
use crate::pda::simulator::AcceptMode;
use crate::pda::Pda;

/// A snapshot of one PDA branch at the current step, suitable for UI display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdaConfigSnapshot {
    /// ID of the active state.
    pub state_id: StateId,
    /// Human-readable name of the active state.
    pub state_name: String,
    /// Portion of the input already consumed.
    pub consumed: String,
    /// Portion of the input not yet consumed.
    pub remaining: String,
    /// Stack contents with the **top** first.
    pub stack: String,
    /// Whether this state is a final (accepting) state.
    pub is_final: bool,
}

/// Internal configuration triple.
type PdaConfig = (StateId, usize, String);

/// An interactive, step-by-step PDA simulator.
///
/// Each call to [`step`][PdaStepper::step] advances all active branches by
/// one transition.  Use [`configurations`][PdaStepper::configurations] to
/// obtain the current set of branches for display.
pub struct PdaStepper<'a> {
    pda: &'a Pda,
    input: String,
    initial_stack: String,
    accept_mode: AcceptMode,
    /// `history[0]` = initial configs; `history[n]` = NEW configs added at step n.
    history: Vec<Vec<PdaConfig>>,
    /// All configs ever active (for deduplication and incremental undo).
    visited: HashSet<PdaConfig>,
    done: bool,
    accepted: bool,
}

impl<'a> PdaStepper<'a> {
    /// Creates a new stepper with the default bottom-of-stack marker `"Z"`.
    pub fn new(pda: &'a Pda, input: &str, accept_mode: AcceptMode) -> Self {
        Self::with_initial_stack(pda, input, accept_mode, "Z")
    }

    /// Creates a new stepper with a custom initial stack string.
    pub fn with_initial_stack(
        pda: &'a Pda,
        input: &str,
        accept_mode: AcceptMode,
        initial_stack: impl Into<String>,
    ) -> Self {
        let initial_stack = initial_stack.into();
        let mut visited: HashSet<PdaConfig> = HashSet::new();
        let initial: Vec<PdaConfig> = if let Some(s) = pda.automaton.initial_state() {
            let cfg = (s, 0usize, initial_stack.clone());
            visited.insert(cfg.clone());
            vec![cfg]
        } else {
            vec![]
        };
        let input_str = input.to_owned();
        let accepted = is_accepting_configs(&initial, input_str.len(), accept_mode, pda);
        let done = initial.is_empty();
        PdaStepper {
            pda,
            input: input_str,
            initial_stack,
            accept_mode,
            history: vec![initial],
            visited,
            done,
            accepted,
        }
    }

    /// Returns snapshots of all currently-active PDA branches.
    ///
    /// Results are sorted by state ID for deterministic ordering.
    pub fn configurations(&self) -> Vec<PdaConfigSnapshot> {
        let bytes = self.input.as_bytes();
        let mut snaps: Vec<PdaConfigSnapshot> = self
            .history
            .last()
            .into_iter()
            .flat_map(|v| v.iter())
            .map(|(state_id, pos, stack)| {
                let state_id = *state_id;
                let pos = *pos;
                let state_name = self
                    .pda
                    .automaton
                    .get_state(state_id)
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| format!("q{}", state_id.0));
                PdaConfigSnapshot {
                    state_id,
                    state_name,
                    consumed: String::from_utf8_lossy(&bytes[..pos]).into_owned(),
                    remaining: String::from_utf8_lossy(&bytes[pos..]).into_owned(),
                    stack: stack.clone(),
                    is_final: self.pda.automaton.is_final_state(state_id),
                }
            })
            .collect();
        snaps.sort_by_key(|s| s.state_id);
        snaps
    }

    /// Returns the current step depth (0 = initial state).
    pub fn current_step(&self) -> usize {
        self.history.len().saturating_sub(1)
    }

    /// Advances the simulation by one transition step.
    pub fn step(&mut self) -> StepOutcome {
        if self.done {
            return self.outcome();
        }

        let current: Vec<PdaConfig> = match self.history.last() {
            Some(c) if !c.is_empty() => c.clone(),
            _ => {
                self.done = true;
                return StepOutcome::Rejected;
            }
        };

        let bytes = self.input.as_bytes();
        let mut next: Vec<PdaConfig> = Vec::new();

        for (state, pos, stack) in current {
            for t in self.pda.transitions_from(state) {
                // Input match.
                let new_pos = if t.input_to_read.is_empty() {
                    pos
                } else {
                    let label = t.input_to_read.as_bytes();
                    if bytes.len() < pos + label.len() || &bytes[pos..pos + label.len()] != label {
                        continue;
                    }
                    pos + label.len()
                };

                // Stack pop match.
                let after_pop = if t.string_to_pop.is_empty() {
                    stack.clone()
                } else {
                    let pop = &t.string_to_pop;
                    if stack.len() < pop.len() || &stack[..pop.len()] != pop {
                        continue;
                    }
                    stack[pop.len()..].to_string()
                };

                // Push new characters onto stack.
                let new_stack = format!("{}{}", t.string_to_push, after_pop);

                let cfg: PdaConfig = (t.to, new_pos, new_stack);
                if self.visited.insert(cfg.clone()) {
                    next.push(cfg);
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

        if is_accepting_configs(&next, self.input.len(), self.accept_mode, self.pda) {
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

    /// Reverts to the previous step, restoring the visited set.
    ///
    /// Returns `false` if already at the initial step.
    pub fn step_back(&mut self) -> bool {
        if self.history.len() <= 1 {
            return false;
        }
        let removed = self.history.pop().unwrap();
        for cfg in removed {
            self.visited.remove(&cfg);
        }
        self.done = false;
        self.accepted = is_accepting_configs(
            self.history.last().unwrap_or(&vec![]),
            self.input.len(),
            self.accept_mode,
            self.pda,
        );
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
    pub fn reset(&mut self) {
        let input = self.input.clone();
        let initial_stack = self.initial_stack.clone();
        *self = PdaStepper::with_initial_stack(self.pda, &input, self.accept_mode, initial_stack);
    }
}

fn is_accepting_configs(
    configs: &[PdaConfig],
    input_len: usize,
    accept_mode: AcceptMode,
    pda: &Pda,
) -> bool {
    configs.iter().any(|(state, pos, stack)| {
        if *pos != input_len {
            return false;
        }
        match accept_mode {
            AcceptMode::FinalState => pda.automaton.is_final_state(*state),
            AcceptMode::EmptyStack => stack.is_empty(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pda::{Pda, PdaTransition};

    /// PDA accepting {aⁿbⁿ | n ≥ 1} by final state.
    fn build_anbn_final_state() -> Pda {
        let mut pda = Pda::new();
        let q0 = pda.add_state("q0");
        let q1 = pda.add_state("q1");
        let q2 = pda.add_state("q2");
        pda.set_initial_state(q0);
        pda.add_final_state(q2);

        pda.add_transition(PdaTransition {
            from: q0,
            to: q0,
            input_to_read: "a".into(),
            string_to_pop: "Z".into(),
            string_to_push: "AZ".into(),
        });
        pda.add_transition(PdaTransition {
            from: q0,
            to: q0,
            input_to_read: "a".into(),
            string_to_pop: "A".into(),
            string_to_push: "AA".into(),
        });
        pda.add_transition(PdaTransition {
            from: q0,
            to: q1,
            input_to_read: "b".into(),
            string_to_pop: "A".into(),
            string_to_push: "".into(),
        });
        pda.add_transition(PdaTransition {
            from: q1,
            to: q1,
            input_to_read: "b".into(),
            string_to_pop: "A".into(),
            string_to_push: "".into(),
        });
        pda.add_transition(PdaTransition {
            from: q1,
            to: q2,
            input_to_read: "".into(),
            string_to_pop: "Z".into(),
            string_to_push: "Z".into(),
        });
        pda
    }

    #[test]
    fn stepper_accepts_ab() {
        let pda = build_anbn_final_state();
        let mut stepper = PdaStepper::new(&pda, "ab", AcceptMode::FinalState);
        let outcome = stepper.run_to_completion(50);
        assert_eq!(outcome, StepOutcome::Accepted);
    }

    #[test]
    fn stepper_rejects_a() {
        let pda = build_anbn_final_state();
        let mut stepper = PdaStepper::new(&pda, "a", AcceptMode::FinalState);
        let outcome = stepper.run_to_completion(50);
        assert_eq!(outcome, StepOutcome::Rejected);
    }

    #[test]
    fn stepper_step_back() {
        let pda = build_anbn_final_state();
        let mut stepper = PdaStepper::new(&pda, "aabb", AcceptMode::FinalState);
        stepper.step();
        stepper.step();
        let step = stepper.current_step();
        assert!(stepper.step_back());
        assert_eq!(stepper.current_step(), step - 1);
        // Step forward again.
        stepper.step();
        assert_eq!(stepper.current_step(), step);
    }

    /// PDA accepting {aⁿbⁿ | n ≥ 1} by empty stack.
    ///
    /// Uses a simpler design: push 'A' for each 'a', pop 'A' for each 'b'.
    /// Accept when stack is empty after consuming all input.
    #[test]
    fn empty_stack_acceptance() {
        let mut pda = Pda::new();
        let q0 = pda.add_state("q0");
        let q1 = pda.add_state("q1");
        pda.set_initial_state(q0);
        // No final states needed — accept by empty stack.

        // Push 'A' for each 'a' (pop initial Z first, then pop A on top of stack).
        pda.add_transition(PdaTransition {
            from: q0,
            to: q0,
            input_to_read: "a".into(),
            string_to_pop: "Z".into(),
            string_to_push: "AZ".into(),
        });
        pda.add_transition(PdaTransition {
            from: q0,
            to: q0,
            input_to_read: "a".into(),
            string_to_pop: "A".into(),
            string_to_push: "AA".into(),
        });
        // Switch to popping on first 'b'.
        pda.add_transition(PdaTransition {
            from: q0,
            to: q1,
            input_to_read: "b".into(),
            string_to_pop: "A".into(),
            string_to_push: "".into(),
        });
        pda.add_transition(PdaTransition {
            from: q1,
            to: q1,
            input_to_read: "b".into(),
            string_to_pop: "A".into(),
            string_to_push: "".into(),
        });
        // Pop the Z bottom-of-stack marker — this empties the stack.
        pda.add_transition(PdaTransition {
            from: q1,
            to: q1,
            input_to_read: "".into(),
            string_to_pop: "Z".into(),
            string_to_push: "".into(),
        });

        let mut stepper = PdaStepper::new(&pda, "ab", AcceptMode::EmptyStack);
        assert_eq!(
            stepper.run_to_completion(50),
            StepOutcome::Accepted,
            "ab should be accepted by empty stack"
        );

        let mut stepper2 = PdaStepper::new(&pda, "aabb", AcceptMode::EmptyStack);
        assert_eq!(
            stepper2.run_to_completion(100),
            StepOutcome::Accepted,
            "aabb should be accepted by empty stack"
        );

        let mut stepper3 = PdaStepper::new(&pda, "aab", AcceptMode::EmptyStack);
        assert_eq!(
            stepper3.run_to_completion(50),
            StepOutcome::Rejected,
            "aab should be rejected by empty stack"
        );
    }
}
