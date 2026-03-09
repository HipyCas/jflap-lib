//! Turing Machine tape implementation.
//!
//! The tape is modelled as a bi-infinite sequence of cells, each holding a
//! single character.  Empty cells contain the blank symbol [`BLANK`]
//! (`'□'` U+25A1), matching the original Java implementation.

/// The blank tape symbol (`□` U+25A1), matching JFLAP's convention.
pub const BLANK: char = '\u{25A1}';

/// A mutable, bi-infinite tape backed by a `Vec<char>`.
///
/// Internally the tape is stored as a `Vec<char>` plus an *offset* that maps
/// logical indices (which can be negative) to physical indices into the vector.
///
/// ```
/// use jflap_rs::turing::tape::Tape;
/// let mut tape = Tape::new("abc");
/// assert_eq!(tape.read(), 'a');
/// tape.move_right();
/// assert_eq!(tape.read(), 'b');
/// tape.write('X');
/// assert_eq!(tape.read(), 'X');
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tape {
    /// Buffer, indexed by `head - left_offset`.
    buffer: Vec<char>,
    /// The logical index of `buffer[0]`.
    left_offset: i64,
    /// Current head position (logical index).
    head: i64,
}

impl Tape {
    /// Creates a new tape from an input string.
    ///
    /// If `input` is empty, the tape is initialised with a single blank cell.
    pub fn new(input: &str) -> Self {
        let buffer: Vec<char> = if input.is_empty() {
            vec![BLANK]
        } else {
            input.chars().collect()
        };
        Tape {
            buffer,
            left_offset: 0,
            head: 0,
        }
    }

    /// Returns the character under the tape head.
    pub fn read(&self) -> char {
        let idx = self.physical_idx(self.head);
        self.buffer[idx]
    }

    /// Writes `ch` to the cell under the tape head.
    pub fn write(&mut self, ch: char) {
        let idx = self.physical_idx(self.head);
        self.buffer[idx] = ch;
    }

    /// Moves the head one cell to the left.
    pub fn move_left(&mut self) {
        self.head -= 1;
        self.ensure_capacity();
    }

    /// Moves the head one cell to the right.
    pub fn move_right(&mut self) {
        self.head += 1;
        self.ensure_capacity();
    }

    /// Returns the current head position (logical index).
    pub fn head_pos(&self) -> i64 {
        self.head
    }

    /// Returns the entire tape contents as a `String`.
    pub fn contents(&self) -> String {
        self.buffer.iter().collect()
    }

    /// Returns the *output* of the tape: the non-blank characters starting
    /// from (and including) the tape head, up to the first blank.
    pub fn output(&self) -> String {
        let mut result = String::new();
        let mut pos = self.head;
        loop {
            let ch = self.read_at(pos);
            if ch == BLANK {
                break;
            }
            result.push(ch);
            pos += 1;
        }
        result
    }

    // --- private helpers ---

    fn physical_idx(&self, logical: i64) -> usize {
        (logical - self.left_offset) as usize
    }

    fn read_at(&self, logical: i64) -> char {
        let phys = logical - self.left_offset;
        if phys < 0 || phys as usize >= self.buffer.len() {
            BLANK
        } else {
            self.buffer[phys as usize]
        }
    }

    /// Grows the buffer so that the current head position has a valid cell.
    fn ensure_capacity(&mut self) {
        if self.head < self.left_offset {
            // Need to extend to the left.
            let extend = (self.left_offset - self.head) as usize;
            let blanks: Vec<char> = vec![BLANK; extend];
            let mut new_buf = blanks;
            new_buf.extend_from_slice(&self.buffer);
            self.buffer = new_buf;
            self.left_offset = self.head;
        } else {
            let phys = self.physical_idx(self.head);
            if phys >= self.buffer.len() {
                // Extend to the right with blanks.
                self.buffer.resize(phys + 1, BLANK);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_write_move() {
        let mut tape = Tape::new("ab");
        assert_eq!(tape.read(), 'a');
        tape.move_right();
        assert_eq!(tape.read(), 'b');
        tape.write('X');
        assert_eq!(tape.read(), 'X');
        // Move back.
        tape.move_left();
        assert_eq!(tape.read(), 'a');
    }

    #[test]
    fn extend_right_with_blank() {
        let mut tape = Tape::new("a");
        tape.move_right();
        assert_eq!(tape.read(), BLANK);
    }

    #[test]
    fn extend_left_with_blank() {
        let mut tape = Tape::new("a");
        tape.move_left();
        assert_eq!(tape.read(), BLANK);
    }

    #[test]
    fn empty_input_gets_blank() {
        let tape = Tape::new("");
        assert_eq!(tape.read(), BLANK);
    }

    #[test]
    fn output_stops_at_blank() {
        let mut tape = Tape::new("hello");
        // Advance past 'h'.
        tape.move_right();
        // Output from 'e' onward (no blank in "ello").
        assert_eq!(tape.output(), "ello");
    }
}
