//! Platform-independent Vietnamese input engine for bikey.
//!
//! Supports Telex and VNI and keeps English words as typed: anything that is
//! not a valid Vietnamese syllable (`class`, `text`) or is a known English
//! word (`these`) stays in English, with no mode switch.
//!
//! The engine does no I/O and holds only the word currently being composed
//! (at most [`MAX_KEYS`] keys).
//!
//! ```
//! use bikey_core::Engine;
//!
//! let mut engine = Engine::default();
//! for c in "tieengs".chars() {
//!     engine.key(c);
//! }
//! assert_eq!(engine.commit(), "tiếng");
//! ```

#![forbid(unsafe_code)]

mod chars;
mod engine;
mod english;
mod syllable;
mod word;

pub use engine::{Config, Engine, MAX_KEYS, MAX_USER_WORDS, Output};
pub use word::{InputMethod, ToneStyle};

/// Canonical keystrokes that type `text` with `method`, e.g. `tiếng` → `tieengs`.
pub fn encode(text: &str, method: InputMethod) -> String {
    word::encode(text, method).into_iter().collect()
}
