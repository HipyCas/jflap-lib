//! TM simulator — deterministic, single-tape.
//!
//! The simulator steps one transition at a time.  It mirrors the behaviour of
//! Java's `TMSimulator` with two acceptance modes:
//!
//! - **Final state** — the machine enters a final (accepting) state.
//! - **Halt** — the machine has no valid transition to take (it halts), which
//!   is considered acceptance if the halting state is a final state.
//!
//! A step limit ([`MAX_STEPS`]) prevents infinite loops.

use crate::automaton::StateId;
use crate::turing::tape::Tape;
use crate::turing::{Direction, Tm, TmTransition};

/// Maximum number of simulation steps before the simulator gives up.
pub const MAX_STEPS: usize = 100_000;

/// How the TM decides acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptMode {
    /// Accept when the machine enters a final state (regardless of halting).
    FinalState,
    /// Accept when the machine halts (no valid transition) **and** the current
    /// state is a final state.
    HaltInFinalState,
}

/// The outcome of a simulation run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimResult {
    /// The input was accepted.
    Accepted,
    /// The input was rejected (machine halted in non-final state, or explicitly
    /// rejected).
    Rejected,
    /// The step limit was reached without a decision.
    StepLimitExceeded,
}

/// Simulates a (deterministic) single-tape [`Tm`].
pub struct TmSimulator<'a> {
    tm: &'a Tm,
    accept_mode: AcceptMode,
    max_steps: usize,
}

impl<'a> TmSimulator<'a> {
    /// Creates a new simulator with the given `accept_mode`.
    pub fn new(tm: &'a Tm, accept_mode: AcceptMode) -> Self {
        TmSimulator {
            tm,
            accept_mode,
            max_steps: MAX_STEPS,
        }
    }

    /// Creates a simulator with a custom step limit.
    pub fn with_max_steps(tm: &'a Tm, accept_mode: AcceptMode, max_steps: usize) -> Self {
        TmSimulator {
            tm,
            accept_mode,
            max_steps,
        }
    }

    /// Returns `true` if the TM accepts `input`.
    pub fn accepts(&self, input: &str) -> bool {
        matches!(self.run(input), SimResult::Accepted)
    }

    /// Runs the TM on `input` and returns the detailed [`SimResult`].
    pub fn run(&self, input: &str) -> SimResult {
        let initial = match self.tm.automaton.initial_state() {
            Some(s) => s,
            None => return SimResult::Rejected,
        };

        let mut tape = Tape::new(input);
        let mut state = initial;

        for _ in 0..self.max_steps {
            // Immediately accepted if we are in a final state (FinalState mode).
            if self.accept_mode == AcceptMode::FinalState && self.tm.automaton.is_final_state(state)
            {
                return SimResult::Accepted;
            }

            let symbol = tape.read();
            let transition = self.find_transition(state, symbol);

            match transition {
                None => {
                    // Machine halts.
                    return if self.tm.automaton.is_final_state(state) {
                        SimResult::Accepted
                    } else {
                        SimResult::Rejected
                    };
                }
                Some(t) => {
                    // Write the new symbol ('~' means no change).
                    let to_write = if t.write == '~' { symbol } else { t.write };
                    tape.write(to_write);

                    // Move the head.
                    match t.direction {
                        Direction::Left => tape.move_left(),
                        Direction::Right => tape.move_right(),
                        Direction::Stay => {}
                    }

                    state = t.to;
                }
            }
        }

        SimResult::StepLimitExceeded
    }

    /// Finds the first applicable transition for the given state and symbol.
    ///
    /// Transitions with `'!'` (negation) patterns are sorted last, matching
    /// JFLAP's priority order.
    fn find_transition(&self, state: StateId, symbol: char) -> Option<&TmTransition> {
        // Collect applicable transitions, prioritising non-negation ones.
        let mut normal: Option<&TmTransition> = None;
        let mut negated: Option<&TmTransition> = None;

        for t in self.tm.transitions_from(state) {
            let read_char = t.read;
            if read_char == '~' {
                // Wildcard — matches anything.
                if normal.is_none() {
                    normal = Some(t);
                }
            } else if read_char == symbol {
                normal = Some(t);
                break; // Exact match wins immediately.
            }
            // '!' negation is not a direct char match; skip for now.
        }

        // Fallback: try negation transitions (read_char starts with '!').
        // Since TmTransition.read is a single char, we encode negation as
        // a separate check: the Java code used '!' as the first char of the
        // *string* field, but here we treat it specially.
        if normal.is_none() {
            for t in self.tm.transitions_from(state) {
                if t.read == '!' {
                    // This is a "not-<next-char>" transition — always matches if
                    // no normal transition was found. (Simplified negation.)
                    negated = Some(t);
                    break;
                }
            }
        }

        normal.or(negated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::turing::tape::BLANK;
    use crate::turing::{Direction, Tm, TmTransition};

    /// Build a TM that accepts strings of the form 0ⁿ1ⁿ (n ≥ 1).
    ///
    /// Uses a dedicated start state so that the empty string is immediately
    /// rejected (no transition out of q_start on blank):
    ///
    ///   q_start: '0' → write 'X', move right, go to q1.
    ///   q0 (loop): 'Y' → stay, move right.
    ///              '0' → write 'X', move right, go to q1.
    ///              BLANK → go to q_accept (all 0s matched with 1s).
    ///   q1: scan right past 0s/Ys to find '1'.
    ///   q2: scan left back to 'X'.
    fn build_0n1n_tm() -> Tm {
        let mut tm = Tm::new();
        let q_start = tm.add_state("q_start"); // initial — ensures n≥1
        let q0 = tm.add_state("q0"); // loop: replace next 0
        let q1 = tm.add_state("q1"); // scan right for 1
        let q2 = tm.add_state("q2"); // scan left back to X
        let q_accept = tm.add_state("q_accept");

        tm.set_initial_state(q_start);
        tm.add_final_state(q_accept);

        // ── q_start: only '0' transitions; blank → halt (reject) ──────────
        tm.add_transition(TmTransition {
            from: q_start,
            to: q1,
            read: '0',
            write: 'X',
            direction: Direction::Right,
        });

        // ── q0: back at left edge after a successful round ─────────────────
        // Skip over Y's (already-matched pairs).
        tm.add_transition(TmTransition {
            from: q0,
            to: q0,
            read: 'Y',
            write: 'Y',
            direction: Direction::Right,
        });
        // Replace the next unmatched '0'.
        tm.add_transition(TmTransition {
            from: q0,
            to: q1,
            read: '0',
            write: 'X',
            direction: Direction::Right,
        });
        // Blank after all Ys means every 0 was matched — accept.
        tm.add_transition(TmTransition {
            from: q0,
            to: q_accept,
            read: BLANK,
            write: BLANK,
            direction: Direction::Stay,
        });

        // ── q1: scan right to find the matching '1' ────────────────────────
        tm.add_transition(TmTransition {
            from: q1,
            to: q1,
            read: '0',
            write: '0',
            direction: Direction::Right,
        });
        tm.add_transition(TmTransition {
            from: q1,
            to: q1,
            read: 'Y',
            write: 'Y',
            direction: Direction::Right,
        });
        // Found the '1' — replace with 'Y', start scanning left.
        tm.add_transition(TmTransition {
            from: q1,
            to: q2,
            read: '1',
            write: 'Y',
            direction: Direction::Left,
        });

        // ── q2: scan left until we reach the 'X' marker ───────────────────
        tm.add_transition(TmTransition {
            from: q2,
            to: q2,
            read: '0',
            write: '0',
            direction: Direction::Left,
        });
        tm.add_transition(TmTransition {
            from: q2,
            to: q2,
            read: 'Y',
            write: 'Y',
            direction: Direction::Left,
        });
        // Found X — move right and resume the main loop.
        tm.add_transition(TmTransition {
            from: q2,
            to: q0,
            read: 'X',
            write: 'X',
            direction: Direction::Right,
        });

        tm
    }

    #[test]
    fn tm_accepts_01() {
        let tm = build_0n1n_tm();
        let sim = TmSimulator::new(&tm, AcceptMode::HaltInFinalState);
        assert!(sim.accepts("01"), "01 should be accepted");
    }

    #[test]
    fn tm_accepts_0011() {
        let tm = build_0n1n_tm();
        let sim = TmSimulator::new(&tm, AcceptMode::HaltInFinalState);
        assert!(sim.accepts("0011"), "0011 should be accepted");
    }

    #[test]
    fn tm_rejects_0() {
        let tm = build_0n1n_tm();
        let sim = TmSimulator::new(&tm, AcceptMode::HaltInFinalState);
        assert!(!sim.accepts("0"), "single 0 should be rejected");
    }

    #[test]
    fn tm_rejects_001() {
        let tm = build_0n1n_tm();
        let sim = TmSimulator::new(&tm, AcceptMode::HaltInFinalState);
        assert!(!sim.accepts("001"), "001 should be rejected");
    }

    #[test]
    fn tm_rejects_empty() {
        let tm = build_0n1n_tm();
        let sim = TmSimulator::new(&tm, AcceptMode::HaltInFinalState);
        assert!(!sim.accepts(""), "empty string should be rejected");
    }
}
