//! FSA simulator — breadth-first NFA simulation.
//!
//! The simulator mirrors the Java `FSAStepByStateSimulator` behaviour:
//! it performs a BFS over all reachable configurations, following both
//! labelled and lambda (ε) transitions.  It also supports JFLAP-style
//! **character-class** labels of the form `[a-z]`.

use std::collections::{HashSet, VecDeque};

use crate::automaton::StateId;
use crate::fsa::Fsa;

/// Simulates an [`Fsa`] on an input string using BFS.
pub struct FsaSimulator<'a> {
    fsa: &'a Fsa,
}

impl<'a> FsaSimulator<'a> {
    /// Creates a new simulator bound to `fsa`.
    pub fn new(fsa: &'a Fsa) -> Self {
        FsaSimulator { fsa }
    }

    /// Returns `true` if `input` is accepted by the automaton.
    pub fn accepts(&self, input: &str) -> bool {
        let initial = match self.fsa.automaton.initial_state() {
            Some(s) => s,
            None => return false,
        };

        // Each item on the queue is (current_state, remaining_input).
        let mut queue: VecDeque<(StateId, usize)> = VecDeque::new();
        let mut visited: HashSet<(StateId, usize)> = HashSet::new();

        queue.push_back((initial, 0));
        visited.insert((initial, 0));

        let bytes = input.as_bytes();

        while let Some((state, pos)) = queue.pop_front() {
            // Accept if we've consumed all input and are in a final state.
            if pos == bytes.len() && self.fsa.automaton.is_final_state(state) {
                return true;
            }

            for t in self.fsa.transitions_from(state) {
                if t.label.is_empty() {
                    // Lambda / ε transition — consume no input.
                    let key = (t.to, pos);
                    if visited.insert(key) {
                        queue.push_back(key);
                    }
                } else if let Some(matched_len) = match_label(&t.label, bytes, pos) {
                    let new_pos = pos + matched_len;
                    let key = (t.to, new_pos);
                    if visited.insert(key) {
                        queue.push_back(key);
                    }
                }
            }
        }

        false
    }
}

/// Attempts to match a transition label against `bytes` starting at `pos`.
///
/// Supports:
/// - Ordinary single-character or multi-character labels.
/// - JFLAP-style character-class ranges such as `[a-z]`.
///
/// Returns the number of bytes consumed on success, or `None` on failure.
pub(crate) fn match_label(label: &str, bytes: &[u8], pos: usize) -> Option<usize> {
    if label.starts_with('[') {
        // Character-class range [x-y] — consumes exactly one character.
        if pos >= bytes.len() {
            return None;
        }
        let ch = bytes[pos] as char;
        // Extract start and end of range, e.g. '[a-z]' → 'a'..'z'
        let chars: Vec<char> = label.chars().collect();
        // Expected format: '[' start '-' end ']'
        if chars.len() == 5 && chars[0] == '[' && chars[2] == '-' && chars[4] == ']' {
            let lo = chars[1];
            let hi = chars[3];
            if ch >= lo && ch <= hi {
                return Some(1);
            }
        }
        None
    } else {
        // Ordinary label — must match byte-for-byte.
        let label_bytes = label.as_bytes();
        if bytes.len() >= pos + label_bytes.len()
            && &bytes[pos..pos + label_bytes.len()] == label_bytes
        {
            Some(label_bytes.len())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsa::{Fsa, FsaTransition};

    /// Helper: build an FSA that accepts exactly the language a*b.
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
    fn accepts_b() {
        let fsa = build_astar_b();
        assert!(FsaSimulator::new(&fsa).accepts("b"));
    }

    #[test]
    fn accepts_aab() {
        let fsa = build_astar_b();
        assert!(FsaSimulator::new(&fsa).accepts("aab"));
    }

    #[test]
    fn rejects_empty() {
        let fsa = build_astar_b();
        assert!(!FsaSimulator::new(&fsa).accepts(""));
    }

    #[test]
    fn rejects_ba() {
        let fsa = build_astar_b();
        assert!(!FsaSimulator::new(&fsa).accepts("ba"));
    }

    #[test]
    fn lambda_transition() {
        // Language: {a, ε} — lambda from q0 to q1 (final), and "a" from q0 to q1.
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
        fsa.add_transition(FsaTransition {
            from: s0,
            to: s1,
            label: "a".into(),
        });
        let sim = FsaSimulator::new(&fsa);
        assert!(sim.accepts(""));
        assert!(sim.accepts("a"));
        assert!(!sim.accepts("b"));
        assert!(!sim.accepts("aa"));
    }

    #[test]
    fn character_range_label() {
        // Accepts exactly one lowercase letter.
        let mut fsa = Fsa::new();
        let s0 = fsa.add_state("q0");
        let s1 = fsa.add_state("q1");
        fsa.set_initial_state(s0);
        fsa.add_final_state(s1);
        fsa.add_transition(FsaTransition {
            from: s0,
            to: s1,
            label: "[a-z]".into(),
        });
        let sim = FsaSimulator::new(&fsa);
        assert!(sim.accepts("a"));
        assert!(sim.accepts("z"));
        assert!(!sim.accepts("A"));
        assert!(!sim.accepts("ab")); // two letters — rejected
    }
}
