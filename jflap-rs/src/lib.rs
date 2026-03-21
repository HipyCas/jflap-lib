//! # jflap-rs
//!
//! A Rust port of the JFLAP 7.0 core automata engine.
//!
//! This crate provides data structures, simulators, and **interactive steppers**
//! for:
//! - **FSA** (Finite State Automata) — including NFA with lambda (ε) transitions
//! - **PDA** (Pushdown Automata) — accept by final state or empty stack
//! - **TM** (Turing Machines) — single-tape, deterministic
//!
//! It also supports loading automata from `.jff` XML files compatible with the
//! JFLAP application.
//!
//! ## Batch simulation
//!
//! ```rust
//! use jflap_rs::fsa::{Fsa, FsaTransition};
//! use jflap_rs::fsa::simulator::FsaSimulator;
//!
//! let mut fsa = Fsa::new();
//! let s0 = fsa.add_state("q0");
//! let s1 = fsa.add_state("q1");
//! fsa.set_initial_state(s0);
//! fsa.add_final_state(s1);
//! fsa.add_transition(FsaTransition { from: s0, to: s1, label: "b".into() });
//! fsa.add_transition(FsaTransition { from: s0, to: s0, label: "a".into() });
//!
//! let sim = FsaSimulator::new(&fsa);
//! assert!(sim.accepts("b"));
//! assert!(sim.accepts("aab"));
//! assert!(!sim.accepts("ba"));
//! assert!(!sim.accepts(""));
//! ```
//!
//! ## Interactive step-by-step simulation (for Tauri UIs)
//!
//! The `stepper` sub-modules expose a step-by-step API where each call to
//! `step()` advances by one transition and returns an [`automaton::StepOutcome`].
//! Call `configurations()` / `configuration()` between steps to obtain
//! UI-renderable snapshots of the current machine state.
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
//! assert_eq!(stepper.step(), StepOutcome::Active);   // "a" consumed
//! assert_eq!(stepper.step(), StepOutcome::Accepted); // "b" consumed
//! ```

pub mod automaton;
pub mod file;
pub mod fsa;
pub mod pda;
pub mod turing;
