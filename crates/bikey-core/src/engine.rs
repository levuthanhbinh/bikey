//! The per-text-field composition state machine used by every platform shim.

use core::fmt;
use std::collections::BTreeSet;

use crate::english;
use crate::syllable::Mode;
use crate::word::{self, InputMethod, ToneStyle, Word};

/// Longest word kept in the composition buffer. Longer input is committed in chunks.
pub const MAX_KEYS: usize = 32;
/// Cap on user-added English words, so a bad config cannot grow memory without bound.
pub const MAX_USER_WORDS: usize = 10_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub method: InputMethod,
    pub tone_style: ToneStyle,
    /// Keep words that are not valid Vietnamese as typed (smart English mode).
    pub auto_english: bool,
    /// Also keep the built-in list of common English words (`these`, `car`, …).
    pub english_exceptions: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            method: InputMethod::Telex,
            tone_style: ToneStyle::Traditional,
            auto_english: true,
            english_exceptions: true,
        }
    }
}

/// What the shim should do after a key: commit `commit` (if non-empty), then
/// show `preedit` as the composition string.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    pub commit: String,
    pub preedit: String,
}

/// Composition state for one input context.
///
/// The shim feeds it word characters with [`Engine::key`]. For any other key
/// (space, punctuation, Enter, arrows, focus change) it calls
/// [`Engine::commit`], inserts the result, then lets the key through.
pub struct Engine {
    config: Config,
    raw: Vec<char>,
    /// Set by Backspace inside a word shown in English, so it does not turn
    /// Vietnamese again (`text` → Backspace shows `tex`, not `tẽ`).
    keep_typed: bool,
    user_words: BTreeSet<String>,
}

// Hand-written so typed text can never end up in logs via `{:?}`.
impl fmt::Debug for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Engine")
            .field("config", &self.config)
            .field("composing", &!self.raw.is_empty())
            .field("user_words", &self.user_words.len())
            .finish_non_exhaustive()
    }
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new(Config::default())
    }
}

impl Engine {
    pub fn new(config: Config) -> Self {
        Engine {
            config,
            raw: Vec::with_capacity(MAX_KEYS),
            keep_typed: false,
            user_words: BTreeSet::new(),
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Takes effect on the current word too, so shims usually commit first.
    pub fn set_config(&mut self, config: Config) {
        self.config = config;
    }

    /// Add a word that should always stay English. Returns `false` (and adds
    /// nothing) if it is not 1–[`MAX_KEYS`] ASCII letters or the list is full.
    pub fn add_english_word(&mut self, word: &str) -> bool {
        let valid = !word.is_empty()
            && word.len() <= MAX_KEYS
            && word.bytes().all(|b| b.is_ascii_alphabetic());
        if !valid || self.user_words.len() >= MAX_USER_WORDS {
            return false;
        }
        self.user_words.insert(word.to_ascii_lowercase());
        true
    }

    pub fn clear_english_words(&mut self) {
        self.user_words.clear();
    }

    /// Whether `c` is part of a word for the current input method. VNI digits
    /// only count inside a word, so `2026` is typed normally.
    pub fn accepts(&self, c: char) -> bool {
        c.is_ascii_alphabetic()
            || (self.config.method == InputMethod::Vni
                && c.is_ascii_digit()
                && !self.raw.is_empty())
    }

    pub fn is_composing(&self) -> bool {
        !self.raw.is_empty()
    }

    /// Feed one character. `None` means the key is not part of a word: the
    /// shim should [`commit`](Engine::commit) and pass the key on.
    pub fn key(&mut self, c: char) -> Option<Output> {
        if !self.accepts(c) {
            return None;
        }
        let mut out = Output::default();
        if self.raw.len() >= MAX_KEYS {
            out.commit = self.commit();
        }
        self.raw.push(c);
        out.preedit = self.preedit();
        Some(out)
    }

    /// Delete the last displayed character. `None` means nothing is being
    /// composed and the shim should pass Backspace to the application.
    pub fn backspace(&mut self) -> Option<Output> {
        if self.raw.is_empty() {
            return None;
        }
        let shown = self.preedit();
        if shown.chars().eq(self.raw.iter().copied()) {
            let converted =
                Word::from_keys(&self.raw, self.config.method).render(self.config.tone_style);
            if !converted.chars().eq(self.raw.iter().copied()) {
                self.keep_typed = true;
            }
            self.raw.pop();
        } else {
            // Re-type what is left so marks and the tone stay where they belong.
            let mut rest: Vec<char> = shown.chars().collect();
            rest.pop();
            let rest: String = rest.into_iter().collect();
            self.raw = word::encode(&rest, self.config.method);
        }
        if self.raw.is_empty() {
            self.keep_typed = false;
        }
        Some(Output {
            commit: String::new(),
            preedit: self.preedit(),
        })
    }

    /// Finish the current word and return the text to insert (may be empty).
    pub fn commit(&mut self) -> String {
        let text = self.render(Mode::Complete);
        self.reset();
        text
    }

    /// Drop the current word without inserting anything (e.g. Escape).
    pub fn reset(&mut self) {
        self.raw.clear();
        self.keep_typed = false;
    }

    /// The exact keys of the current word, to put them back if the user asks.
    pub fn typed(&self) -> String {
        self.raw.iter().collect()
    }

    /// The composition string to display right now.
    pub fn preedit(&self) -> String {
        self.render(Mode::Prefix)
    }

    fn render(&self, mode: Mode) -> String {
        let raw: String = self.raw.iter().collect();
        if self.keep_typed {
            return raw;
        }
        let word = Word::from_keys(&self.raw, self.config.method);
        let vietnamese = word.render(self.config.tone_style);
        if vietnamese == raw || !self.config.auto_english {
            return vietnamese;
        }
        let lower = raw.to_ascii_lowercase();
        let listed = (self.config.english_exceptions && english::contains(&lower))
            || self.user_words.contains(&lower);
        if listed {
            raw
        } else if word.cancelled() || word.is_valid(mode) {
            vietnamese
        } else {
            raw
        }
    }
}
