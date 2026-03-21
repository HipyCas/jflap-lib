//! PDA simulator — BFS over configurations.
//!
//! Each *configuration* is `(state, remaining_input_index, stack)`.
//! The simulation is bounded at [`MAX_CONFIGS`] to avoid unbounded loops on
//! non-halting inputs.

use std::collections::{HashSet, VecDeque};

use crate::automaton::StateId;
use crate::pda::Pda;

/// Maximum number of configurations to explore before giving up.
///
/// This mirrors the behaviour of the original Java code which shows a dialog
/// after 10 000 configurations.
pub const MAX_CONFIGS: usize = 10_000;

/// How the PDA decides acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptMode {
    /// Accept when all input is consumed **and** the current state is final.
    FinalState,
    /// Accept when all input is consumed **and** the stack is empty.
    EmptyStack,
}

/// A PDA configuration (snapshot) used during simulation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Config {
    state: StateId,
    /// Index into the original input string (byte offset).
    pos: usize,
    /// Stack contents; the *first* character is the top.
    stack: String,
}

/// Simulates a [`Pda`] on an input string using BFS.
pub struct PdaSimulator<'a> {
    pda: &'a Pda,
    /// Initial stack content.  JFLAP pushes `"Z"` as a bottom-of-stack marker.
    initial_stack: String,
    accept_mode: AcceptMode,
}

impl<'a> PdaSimulator<'a> {
    /// Creates a new simulator using `accept_mode` to determine acceptance.
    ///
    /// The initial stack contains a single bottom-of-stack marker `"Z"`, matching
    /// the behaviour of the original Java `PDAStepByStateSimulator`.
    pub fn new(pda: &'a Pda, accept_mode: AcceptMode) -> Self {
        PdaSimulator {
            pda,
            initial_stack: "Z".into(),
            accept_mode,
        }
    }

    /// Creates a simulator with a custom initial stack string.
    pub fn with_initial_stack(
        pda: &'a Pda,
        accept_mode: AcceptMode,
        initial_stack: impl Into<String>,
    ) -> Self {
        PdaSimulator {
            pda,
            initial_stack: initial_stack.into(),
            accept_mode,
        }
    }

    /// Returns `true` if the PDA accepts `input`.
    ///
    /// Returns `false` if the input is rejected **or** the simulation exceeds
    /// [`MAX_CONFIGS`] without finding an accepting configuration.
    pub fn accepts(&self, input: &str) -> bool {
        let initial = match self.pda.automaton.initial_state() {
            Some(s) => s,
            None => return false,
        };

        let bytes = input.as_bytes();
        let mut queue: VecDeque<Config> = VecDeque::new();
        let mut visited: HashSet<Config> = HashSet::new();
        let mut explored: usize = 0;

        let init_config = Config {
            state: initial,
            pos: 0,
            stack: self.initial_stack.clone(),
        };
        queue.push_back(init_config.clone());
        visited.insert(init_config);

        while let Some(cfg) = queue.pop_front() {
            explored += 1;
            if explored > MAX_CONFIGS {
                return false;
            }

            if self.is_accepting(&cfg, bytes.len()) {
                return true;
            }

            for t in self.pda.transitions_from(cfg.state) {
                // Check input match.
                let new_pos = if t.input_to_read.is_empty() {
                    cfg.pos // lambda — no input consumed
                } else {
                    let label = t.input_to_read.as_bytes();
                    if bytes.len() < cfg.pos + label.len() {
                        continue;
                    }
                    if &bytes[cfg.pos..cfg.pos + label.len()] != label {
                        continue;
                    }
                    cfg.pos + label.len()
                };

                // Check stack match (pop).
                let new_stack = if t.string_to_pop.is_empty() {
                    // Nothing to pop — stack unchanged by pop phase.
                    cfg.stack.clone()
                } else {
                    let pop = &t.string_to_pop;
                    if cfg.stack.len() < pop.len() {
                        continue;
                    }
                    if &cfg.stack[..pop.len()] != pop {
                        continue;
                    }
                    // Remove popped characters from top of stack.
                    cfg.stack[pop.len()..].to_string()
                };

                // Push new characters.
                let pushed_stack = format!("{}{}", t.string_to_push, new_stack);

                let new_cfg = Config {
                    state: t.to,
                    pos: new_pos,
                    stack: pushed_stack,
                };
                if visited.insert(new_cfg.clone()) {
                    queue.push_back(new_cfg);
                }
            }
        }

        false
    }

    fn is_accepting(&self, cfg: &Config, input_len: usize) -> bool {
        if cfg.pos != input_len {
            return false;
        }
        match self.accept_mode {
            AcceptMode::FinalState => self.pda.automaton.is_final_state(cfg.state),
            AcceptMode::EmptyStack => cfg.stack.is_empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pda::{Pda, PdaTransition};

    /// Build a PDA that accepts the language {aⁿbⁿ | n ≥ 1}
    /// using final-state acceptance.
    ///
    /// States: q0 (initial) --a,Z;AZ--> q0
    ///                       --a,A;AA--> q0
    ///                       --b,A;λ --> q1
    ///         q1             --b,A;λ --> q1
    ///                       --λ,Z;Z  --> q2 (final)
    fn build_anbn() -> Pda {
        let mut pda = Pda::new();
        let q0 = pda.add_state("q0");
        let q1 = pda.add_state("q1");
        let q2 = pda.add_state("q2");
        pda.set_initial_state(q0);
        pda.add_final_state(q2);

        // Read 'a': push A on top of stack marker Z, or on top of A.
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
        // Start reading b's.
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
        // Lambda: accept when stack has only Z left.
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
    fn anbn_accepts() {
        let pda = build_anbn();
        let sim = PdaSimulator::new(&pda, AcceptMode::FinalState);
        assert!(sim.accepts("ab"), "ab should be accepted");
        assert!(sim.accepts("aabb"), "aabb should be accepted");
        assert!(sim.accepts("aaabbb"), "aaabbb should be accepted");
    }

    #[test]
    fn anbn_rejects() {
        let pda = build_anbn();
        let sim = PdaSimulator::new(&pda, AcceptMode::FinalState);
        assert!(!sim.accepts(""), "empty string rejected");
        assert!(!sim.accepts("a"), "single a rejected");
        assert!(!sim.accepts("b"), "single b rejected");
        assert!(!sim.accepts("ba"), "ba rejected");
        assert!(!sim.accepts("aab"), "aab rejected");
        assert!(!sim.accepts("abb"), "abb rejected");
    }

    /// PDA accepting {aⁿbⁿ | n ≥ 1} by **empty stack** acceptance.
    ///
    /// Push 'A' for each 'a', pop 'A' for each 'b'.  After the last 'b',
    /// pop the bottom-of-stack 'Z' via a lambda transition, leaving the stack
    /// empty.
    fn build_anbn_empty_stack() -> Pda {
        let mut pda = Pda::new();
        let q0 = pda.add_state("q0");
        let q1 = pda.add_state("q1");
        pda.set_initial_state(q0);
        // No final states — acceptance is by empty stack.

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
        // Lambda: pop the bottom Z to empty the stack.
        pda.add_transition(PdaTransition {
            from: q1,
            to: q1,
            input_to_read: "".into(),
            string_to_pop: "Z".into(),
            string_to_push: "".into(),
        });
        pda
    }

    #[test]
    fn anbn_empty_stack_accepts() {
        let pda = build_anbn_empty_stack();
        let sim = PdaSimulator::new(&pda, AcceptMode::EmptyStack);
        assert!(sim.accepts("ab"), "ab accepted by empty stack");
        assert!(sim.accepts("aabb"), "aabb accepted by empty stack");
        assert!(sim.accepts("aaabbb"), "aaabbb accepted by empty stack");
    }

    #[test]
    fn anbn_empty_stack_rejects() {
        let pda = build_anbn_empty_stack();
        let sim = PdaSimulator::new(&pda, AcceptMode::EmptyStack);
        assert!(!sim.accepts(""), "empty string rejected");
        assert!(!sim.accepts("a"), "a rejected");
        assert!(!sim.accepts("aab"), "aab rejected");
    }
}
