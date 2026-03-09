//! # jflap-rs
//!
//! A Rust port of the JFLAP 7.0 core automata engine.
//!
//! This crate provides data structures and simulators for:
//! - **FSA** (Finite State Automata) — including NFA with lambda (ε) transitions
//! - **PDA** (Pushdown Automata) — accept by final state or empty stack
//! - **TM** (Turing Machines) — single-tape, deterministic
//!
//! It also supports loading automata from `.jff` XML files compatible with the
//! JFLAP application.
//!
//! ## Quick Start
//!
//! ```rust
//! use jflap_rs::fsa::{Fsa, FsaTransition};
//! use jflap_rs::fsa::simulator::FsaSimulator;
//!
//! // Build a simple FSA that accepts strings of the form a*b
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

pub mod automaton;
pub mod file;
pub mod fsa;
pub mod pda;
pub mod turing;
