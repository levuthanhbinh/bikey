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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Language {
    Vietnamese,
    English,
}

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
    /// The next word looks like code or an address (`obj.|dir`): keep it as typed.
    literal_next: bool,
    /// The last thing typed was a digit outside a word, so `.` is a decimal point.
    after_digit: bool,
    /// The language of the last word that showed one; decides ambiguous words.
    previous: Option<Language>,
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
            literal_next: false,
            after_digit: false,
            previous: None,
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
            let literal = self.literal_next;
            out.commit = self.commit();
            self.literal_next = literal;
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

    /// Finish the current word because `c` was typed, returning the word's final
    /// text (empty if none). Also call this for a character typed outside a word:
    /// it decides whether the next word looks like code and stays as typed.
    ///
    /// `_` and `@` always mean code or an address, and `_` also restores the word
    /// before it (`dir_name`). `.`, `/`, `\`, `:`, `-` and digits only count after
    /// a word that is not Vietnamese (`obj.dir`, but `và/hoặc` and `Hà-Nội` convert).
    pub fn commit_with(&mut self, c: char) -> String {
        let typed = self.typed();
        let mut text = self.render(Mode::Complete);
        let had_word = !typed.is_empty();
        let vietnamese = had_word && !self.literal_next && {
            let word = Word::from_keys(&self.raw, self.config.method);
            word.is_valid(Mode::Complete) && text == word.render(self.config.tone_style)
        };
        let smart = self.config.auto_english;
        if smart && c == '_' {
            text.clone_from(&typed);
        }
        let code_word = had_word && !vietnamese;
        if had_word {
            let word = Word::from_keys(&self.raw, self.config.method);
            let lower = typed.to_ascii_lowercase();
            let language = if self.literal_next || c == '_' || word.cancelled() {
                Some(Language::English)
            } else if text != typed {
                Some(Language::Vietnamese)
            } else if self.listed(&lower)
                || english::signals_english(&lower)
                || !word.is_valid(Mode::Complete)
            {
                Some(Language::English)
            } else {
                // A plain word like `nay` or `to` could be either: look further back.
                None
            };
            self.previous = language.or(self.previous);
        }
        self.literal_next = smart
            && match c {
                '_' | '@' => true,
                '.' if !had_word && self.after_digit => false,
                '.' | '/' | '\\' => !had_word || code_word,
                ':' | '-' => code_word || (!had_word && self.literal_next),
                c if c.is_ascii_digit() => code_word || (!had_word && self.literal_next),
                _ => false,
            };
        self.after_digit = !had_word && (c.is_ascii_digit() || (c == '.' && self.after_digit));
        self.raw.clear();
        self.keep_typed = false;
        text
    }

    /// Forget what came before (Enter, arrows, Backspace): whether the next word
    /// looks like code, and the language of the previous word.
    pub fn clear_context(&mut self) {
        self.literal_next = false;
        self.after_digit = false;
        self.previous = None;
    }

    /// The user asked for the exact keys back, so the text is English from here.
    pub fn mark_previous_english(&mut self) {
        self.previous = Some(Language::English);
    }

    /// Drop the current word without inserting anything (e.g. Escape).
    pub fn reset(&mut self) {
        self.raw.clear();
        self.keep_typed = false;
        self.clear_context();
    }

    /// The exact keys of the current word, to put them back if the user asks.
    pub fn typed(&self) -> String {
        self.raw.iter().collect()
    }

    /// The composition string to display right now.
    pub fn preedit(&self) -> String {
        self.render(Mode::Prefix)
    }

    /// Whether this lowercase word stays English. Ambiguous words follow the
    /// previous word's language, and otherwise use the built-in list.
    fn listed(&self, lower: &str) -> bool {
        if self.user_words.contains(lower) {
            return true;
        }
        if !self.config.english_exceptions {
            return false;
        }
        match self.previous {
            Some(Language::Vietnamese) if english::is_ambiguous(lower) => false,
            Some(Language::English) if english::is_ambiguous(lower) => true,
            _ => english::contains(lower),
        }
    }

    fn render(&self, mode: Mode) -> String {
        let raw: String = self.raw.iter().collect();
        if self.keep_typed || self.literal_next {
            return raw;
        }
        let word = Word::from_keys(&self.raw, self.config.method);
        let vietnamese = word.render(self.config.tone_style);
        if vietnamese == raw || !self.config.auto_english {
            return vietnamese;
        }
        let lower = raw.to_ascii_lowercase();
        if self.listed(&lower) {
            raw
        } else if word.cancelled() || word.is_valid(mode) {
            vietnamese
        } else {
            raw
        }
    }
}
