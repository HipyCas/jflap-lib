//! XML parser for JFLAP `.jff` files.
//!
//! ## File format overview
//!
//! A `.jff` file is an XML document with the following top-level structure:
//!
//! ```xml
//! <?xml version="1.0"?>
//! <structure>
//!   <type>fa</type>           <!-- or "pda", "turing" -->
//!   <automaton>
//!     <state id="0" name="q0">
//!       <x>100.0</x>
//!       <y>150.0</y>
//!       <initial/>            <!-- marks the initial state -->
//!     </state>
//!     <state id="1" name="q1">
//!       <x>200.0</x>
//!       <y>150.0</y>
//!       <final/>              <!-- marks a final state -->
//!     </state>
//!     <!-- FSA transition -->
//!     <transition>
//!       <from>0</from>
//!       <to>1</to>
//!       <read>a</read>        <!-- empty = lambda -->
//!     </transition>
//!   </automaton>
//! </structure>
//! ```
//!
//! For PDA transitions the `<read>` tag is supplemented by `<pop>` and
//! `<push>`.  For TM transitions it is supplemented by `<write>` and
//! `<move>` (direction: `L`, `R`, or `S`).

use std::collections::HashMap;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::automaton::{Automaton, State, StateId};
use crate::fsa::{Fsa, FsaTransition};
use crate::pda::{Pda, PdaTransition};
use crate::turing::{Direction, Tm, TmTransition};

// ── public API ───────────────────────────────────────────────────────────────

/// The parsed result of a `.jff` file.
#[derive(Debug)]
pub enum AutomatonKind {
    Fsa(Fsa),
    Pda(Pda),
    Turing(Tm),
}

/// Error type returned when parsing fails.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("XML read error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Unknown automaton type: {0}")]
    UnknownType(String),
    #[error("Missing required element: {0}")]
    Missing(String),
    #[error("Invalid value for {field}: {value}")]
    InvalidValue { field: String, value: String },
    #[error("UTF-8 decode error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
}

/// Loads an automaton from a `.jff` file on disk.
pub fn load_file(path: impl AsRef<Path>) -> Result<AutomatonKind, ParseError> {
    let content = std::fs::read_to_string(path)?;
    load_str(&content)
}

/// Loads an automaton from an in-memory XML string.
pub fn load_str(xml: &str) -> Result<AutomatonKind, ParseError> {
    let doc = parse_document(xml)?;
    build_automaton(doc)
}

// ── internal representation ───────────────────────────────────────────────────

/// A raw, untyped representation of the parsed XML.  We parse the whole
/// document first and then interpret it, to keep the parsing logic simple.
#[derive(Default)]
struct Document {
    automaton_type: String,
    states: Vec<RawState>,
    transitions: Vec<HashMap<String, String>>,
}

#[derive(Default)]
struct RawState {
    id: u32,
    name: String,
    x: f64,
    y: f64,
    is_initial: bool,
    is_final: bool,
    label: Option<String>,
}

// ── XML parsing ───────────────────────────────────────────────────────────────

fn parse_document(xml: &str) -> Result<Document, ParseError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut doc = Document::default();

    // Simple stack-based state machine.
    enum Context {
        Root,
        Structure,
        Type,
        Automaton,
        State(RawState),
        StateX,
        StateY,
        StateLabel,
        Transition(HashMap<String, String>),
        TransitionField(String),
    }

    let mut stack: Vec<Context> = vec![Context::Root];
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) => {
                let name = std::str::from_utf8(e.name().as_ref())?.to_owned();
                match stack.last_mut() {
                    Some(Context::Root) if name == "structure" => {
                        stack.push(Context::Structure);
                    }
                    Some(Context::Structure) if name == "type" => {
                        stack.push(Context::Type);
                    }
                    Some(Context::Structure) | Some(Context::Automaton) if name == "automaton" => {
                        stack.push(Context::Automaton);
                    }
                    Some(Context::Automaton) if name == "state" => {
                        let mut rs = RawState::default();
                        for attr in e.attributes().flatten() {
                            let key = std::str::from_utf8(attr.key.as_ref())?.to_owned();
                            let val = std::str::from_utf8(attr.value.as_ref())?.to_owned();
                            match key.as_str() {
                                "id" => {
                                    rs.id = val.trim().parse().map_err(|_| {
                                        ParseError::InvalidValue {
                                            field: "state id".into(),
                                            value: val.clone(),
                                        }
                                    })?
                                }
                                "name" => rs.name = val,
                                _ => {}
                            }
                        }
                        stack.push(Context::State(rs));
                    }
                    Some(Context::State(_)) if name == "x" => {
                        stack.push(Context::StateX);
                    }
                    Some(Context::State(_)) if name == "y" => {
                        stack.push(Context::StateY);
                    }
                    Some(Context::State(_)) if name == "label" => {
                        stack.push(Context::StateLabel);
                    }
                    Some(Context::Automaton) if name == "transition" => {
                        stack.push(Context::Transition(HashMap::new()));
                    }
                    Some(Context::Transition(_)) => {
                        stack.push(Context::TransitionField(name));
                    }
                    _ => {
                        // Unknown or irrelevant element — skip its children by
                        // pushing a dummy context.
                        stack.push(Context::Root);
                    }
                }
            }
            Event::Empty(ref e) => {
                // Self-closing tags like <initial/> and <final/>.
                let name = std::str::from_utf8(e.name().as_ref())?.to_owned();
                if let Some(Context::State(ref mut rs)) = stack.last_mut() {
                    match name.as_str() {
                        "initial" => rs.is_initial = true,
                        "final" => rs.is_final = true,
                        _ => {}
                    }
                }
            }
            Event::Text(ref e) => {
                let text = e.unescape().unwrap_or_default().trim().to_string();
                if text.is_empty() {
                    buf.clear();
                    continue;
                }
                // Determine what to do based on the top of the stack, then
                // act — splitting the borrow into a pre-check and an action to
                // avoid simultaneous mutable+immutable borrows of `stack`.
                let top_kind = match stack.last() {
                    Some(Context::Type) => 0u8,
                    Some(Context::StateX) => 1,
                    Some(Context::StateY) => 2,
                    Some(Context::StateLabel) => 3,
                    Some(Context::TransitionField(_)) => 4,
                    _ => 255,
                };
                let field_name_opt: Option<String> = if top_kind == 4 {
                    stack.last().and_then(|c| {
                        if let Context::TransitionField(f) = c {
                            Some(f.clone())
                        } else {
                            None
                        }
                    })
                } else {
                    None
                };
                let parent_idx = stack.len().saturating_sub(2);
                match top_kind {
                    0 => {
                        doc.automaton_type = text;
                    }
                    1 => {
                        if let Some(Context::State(ref mut rs)) = stack.get_mut(parent_idx) {
                            rs.x = text.parse().unwrap_or(0.0);
                        }
                    }
                    2 => {
                        if let Some(Context::State(ref mut rs)) = stack.get_mut(parent_idx) {
                            rs.y = text.parse().unwrap_or(0.0);
                        }
                    }
                    3 => {
                        if let Some(Context::State(ref mut rs)) = stack.get_mut(parent_idx) {
                            rs.label = Some(text);
                        }
                    }
                    4 => {
                        if let Some(field) = field_name_opt {
                            if let Some(Context::Transition(ref mut map)) =
                                stack.get_mut(parent_idx)
                            {
                                map.insert(field, text);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::End(_) => match stack.pop() {
                Some(Context::State(rs)) => {
                    doc.states.push(rs);
                }
                Some(Context::Transition(map)) => {
                    doc.transitions.push(map);
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(doc)
}

// ── builder ───────────────────────────────────────────────────────────────────

fn build_automaton(doc: Document) -> Result<AutomatonKind, ParseError> {
    match doc.automaton_type.trim() {
        "fa" => build_fsa(doc).map(AutomatonKind::Fsa),
        "pda" => build_pda(doc).map(AutomatonKind::Pda),
        "turing" | "tm" => build_tm(doc).map(AutomatonKind::Turing),
        other => Err(ParseError::UnknownType(other.to_owned())),
    }
}

/// Maps the raw numeric ID used in XML to the [`StateId`] actually used in
/// the automaton, and populates state data.
fn populate_states(
    auto: &mut Automaton,
    raw_states: &[RawState],
) -> Result<HashMap<u32, StateId>, ParseError> {
    let mut id_map: HashMap<u32, StateId> = HashMap::new();
    for rs in raw_states {
        let sid = StateId(rs.id);
        let mut state = State::new(sid, rs.name.clone());
        state.x = rs.x;
        state.y = rs.y;
        state.label = rs.label.clone();
        auto.add_state_with_id(state);
        id_map.insert(rs.id, sid);

        if rs.is_initial {
            auto.set_initial_state(sid);
        }
        if rs.is_final {
            auto.add_final_state(sid);
        }
    }
    Ok(id_map)
}

fn resolve(map: &HashMap<u32, StateId>, raw: &str, field: &str) -> Result<StateId, ParseError> {
    let n: u32 = raw.trim().parse().map_err(|_| ParseError::InvalidValue {
        field: field.to_owned(),
        value: raw.to_owned(),
    })?;
    map.get(&n)
        .copied()
        .ok_or_else(|| ParseError::InvalidValue {
            field: field.to_owned(),
            value: raw.to_owned(),
        })
}

fn build_fsa(doc: Document) -> Result<Fsa, ParseError> {
    let mut fsa = Fsa::new();
    let id_map = populate_states(&mut fsa.automaton, &doc.states)?;

    for t in &doc.transitions {
        let from_raw = t.get("from").map(String::as_str).unwrap_or("");
        let to_raw = t.get("to").map(String::as_str).unwrap_or("");
        let label = t.get("read").cloned().unwrap_or_default();

        let from = resolve(&id_map, from_raw, "from")?;
        let to = resolve(&id_map, to_raw, "to")?;

        fsa.add_transition(FsaTransition { from, to, label });
    }

    Ok(fsa)
}

fn build_pda(doc: Document) -> Result<Pda, ParseError> {
    let mut pda = Pda::new();
    let id_map = populate_states(&mut pda.automaton, &doc.states)?;

    for t in &doc.transitions {
        let from_raw = t.get("from").map(String::as_str).unwrap_or("");
        let to_raw = t.get("to").map(String::as_str).unwrap_or("");

        let from = resolve(&id_map, from_raw, "from")?;
        let to = resolve(&id_map, to_raw, "to")?;

        pda.add_transition(PdaTransition {
            from,
            to,
            input_to_read: t.get("read").cloned().unwrap_or_default(),
            string_to_pop: t.get("pop").cloned().unwrap_or_default(),
            string_to_push: t.get("push").cloned().unwrap_or_default(),
        });
    }

    Ok(pda)
}

fn build_tm(doc: Document) -> Result<Tm, ParseError> {
    let mut tm = Tm::new();
    let id_map = populate_states(&mut tm.automaton, &doc.states)?;

    for t in &doc.transitions {
        let from_raw = t.get("from").map(String::as_str).unwrap_or("");
        let to_raw = t.get("to").map(String::as_str).unwrap_or("");

        let from = resolve(&id_map, from_raw, "from")?;
        let to = resolve(&id_map, to_raw, "to")?;

        let read = t
            .get("read")
            .and_then(|s| s.chars().next())
            .unwrap_or(crate::turing::tape::BLANK);
        let write = t.get("write").and_then(|s| s.chars().next()).unwrap_or('~');
        let move_str = t.get("move").map(String::as_str).unwrap_or("R");
        let direction = move_str
            .parse::<Direction>()
            .map_err(|_| ParseError::InvalidValue {
                field: "move".into(),
                value: move_str.to_owned(),
            })?;

        tm.add_transition(TmTransition {
            from,
            to,
            read,
            write,
            direction,
        });
    }

    Ok(tm)
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const FSA_XML: &str = r#"<?xml version="1.0"?>
<structure>
  <type>fa</type>
  <automaton>
    <state id="0" name="q0">
      <x>100</x><y>100</y>
      <initial/>
    </state>
    <state id="1" name="q1">
      <x>200</x><y>100</y>
      <final/>
    </state>
    <transition>
      <from>0</from><to>0</to><read>a</read>
    </transition>
    <transition>
      <from>0</from><to>1</to><read>b</read>
    </transition>
  </automaton>
</structure>"#;

    const PDA_XML: &str = r#"<?xml version="1.0"?>
<structure>
  <type>pda</type>
  <automaton>
    <state id="0" name="q0"><x>0</x><y>0</y><initial/></state>
    <state id="1" name="q1"><x>1</x><y>0</y></state>
    <state id="2" name="q2"><x>2</x><y>0</y><final/></state>
    <transition><from>0</from><to>0</to><read>a</read><pop>Z</pop><push>AZ</push></transition>
    <transition><from>0</from><to>0</to><read>a</read><pop>A</pop><push>AA</push></transition>
    <transition><from>0</from><to>1</to><read>b</read><pop>A</pop><push></push></transition>
    <transition><from>1</from><to>1</to><read>b</read><pop>A</pop><push></push></transition>
    <transition><from>1</from><to>2</to><read></read><pop>Z</pop><push>Z</push></transition>
  </automaton>
</structure>"#;

    const TM_XML: &str = r#"<?xml version="1.0"?>
<structure>
  <type>turing</type>
  <automaton>
    <state id="0" name="q0"><x>0</x><y>0</y><initial/></state>
    <state id="1" name="q1"><x>1</x><y>0</y><final/></state>
    <transition><from>0</from><to>0</to><read>a</read><write>X</write><move>R</move></transition>
    <transition><from>0</from><to>1</to><read>□</read><write>□</write><move>S</move></transition>
  </automaton>
</structure>"#;

    #[test]
    fn parse_fsa() {
        let result = load_str(FSA_XML).expect("FSA parse should succeed");
        match result {
            AutomatonKind::Fsa(fsa) => {
                assert_eq!(fsa.automaton.states().count(), 2);
                assert!(fsa.automaton.initial_state().is_some());
                assert_eq!(fsa.automaton.final_states().len(), 1);
                assert_eq!(fsa.transitions.len(), 2);
            }
            other => panic!("Expected FSA, got {:?}", other),
        }
    }

    #[test]
    fn parse_pda() {
        let result = load_str(PDA_XML).expect("PDA parse should succeed");
        match result {
            AutomatonKind::Pda(pda) => {
                assert_eq!(pda.automaton.states().count(), 3);
                assert_eq!(pda.transitions.len(), 5);
            }
            other => panic!("Expected PDA, got {:?}", other),
        }
    }

    #[test]
    fn parse_tm() {
        let result = load_str(TM_XML).expect("TM parse should succeed");
        match result {
            AutomatonKind::Turing(tm) => {
                assert_eq!(tm.automaton.states().count(), 2);
                assert_eq!(tm.transitions.len(), 2);
            }
            other => panic!("Expected TM, got {:?}", other),
        }
    }

    #[test]
    fn parse_fsa_simulate() {
        use crate::fsa::simulator::FsaSimulator;
        let kind = load_str(FSA_XML).expect("should parse");
        if let AutomatonKind::Fsa(fsa) = kind {
            let sim = FsaSimulator::new(&fsa);
            assert!(sim.accepts("b"));
            assert!(sim.accepts("aab"));
            assert!(!sim.accepts(""));
            assert!(!sim.accepts("ba"));
        } else {
            panic!("Expected FSA");
        }
    }

    #[test]
    fn parse_pda_simulate() {
        use crate::pda::simulator::{AcceptMode, PdaSimulator};
        let kind = load_str(PDA_XML).expect("should parse");
        if let AutomatonKind::Pda(pda) = kind {
            let sim = PdaSimulator::new(&pda, AcceptMode::FinalState);
            assert!(sim.accepts("ab"));
            assert!(sim.accepts("aabb"));
            assert!(!sim.accepts(""));
            assert!(!sim.accepts("aab"));
        } else {
            panic!("Expected PDA");
        }
    }
}
