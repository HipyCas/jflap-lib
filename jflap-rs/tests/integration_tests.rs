//! Integration tests: load automata from `.jff` files and simulate inputs.

use jflap_rs::file::{load_file, AutomatonKind};
use jflap_rs::fsa::simulator::FsaSimulator;
use jflap_rs::pda::simulator::{AcceptMode as PdaAcceptMode, PdaSimulator};
use jflap_rs::turing::simulator::{AcceptMode as TmAcceptMode, TmSimulator};

// ── helpers ───────────────────────────────────────────────────────────────────

fn resource(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/resources")
        .join(name)
}

// ── FSA ───────────────────────────────────────────────────────────────────────

#[test]
fn fsa_load_and_simulate() {
    let kind = load_file(resource("fsa_astar_bb.jff")).expect("failed to load FSA");
    let fsa = match kind {
        AutomatonKind::Fsa(f) => f,
        other => panic!("Expected FSA, got {other:?}"),
    };

    let sim = FsaSimulator::new(&fsa);

    // Language: a*bb
    assert!(sim.accepts("bb"), "bb accepted");
    assert!(sim.accepts("abb"), "abb accepted");
    assert!(sim.accepts("aabb"), "aabb accepted");
    assert!(!sim.accepts(""), "empty rejected");
    assert!(!sim.accepts("b"), "single b rejected");
    assert!(!sim.accepts("bbb"), "bbb rejected");
    assert!(!sim.accepts("ab"), "ab rejected");
    assert!(!sim.accepts("ba"), "ba rejected");
}

// ── PDA ───────────────────────────────────────────────────────────────────────

#[test]
fn pda_load_and_simulate() {
    let kind = load_file(resource("pda_anbn.jff")).expect("failed to load PDA");
    let pda = match kind {
        AutomatonKind::Pda(p) => p,
        other => panic!("Expected PDA, got {other:?}"),
    };

    let sim = PdaSimulator::new(&pda, PdaAcceptMode::FinalState);

    // Language: aⁿbⁿ, n ≥ 1
    assert!(sim.accepts("ab"), "ab accepted");
    assert!(sim.accepts("aabb"), "aabb accepted");
    assert!(sim.accepts("aaabbb"), "aaabbb accepted");
    assert!(!sim.accepts(""), "empty rejected");
    assert!(!sim.accepts("a"), "a rejected");
    assert!(!sim.accepts("b"), "b rejected");
    assert!(!sim.accepts("aab"), "aab rejected");
    assert!(!sim.accepts("ba"), "ba rejected");
}

// ── TM ────────────────────────────────────────────────────────────────────────

#[test]
fn tm_load_and_simulate() {
    let kind = load_file(resource("tm_replace_a_with_b.jff")).expect("failed to load TM");
    let tm = match kind {
        AutomatonKind::Turing(t) => t,
        other => panic!("Expected TM, got {other:?}"),
    };

    // This TM accepts all inputs (replaces a→b until blank, then accepts).
    let sim = TmSimulator::new(&tm, TmAcceptMode::HaltInFinalState);
    assert!(sim.accepts(""), "empty accepted");
    assert!(sim.accepts("a"), "a accepted");
    assert!(sim.accepts("aaa"), "aaa accepted");
}
