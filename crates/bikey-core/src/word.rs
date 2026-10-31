//! A word being composed: raw keys are replayed into letters + one tone.

use crate::chars::{self, Mark, Tone};
use crate::syllable::{self, Mode};

/// Keyboard convention used to type marks and tones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputMethod {
    /// `s f r x j` tones, `z` clears, `aa ee oo` circumflex, `w` horn/breve, `dd` = đ.
    #[default]
    Telex,
    /// `1-5` tones, `0` clears, `6` circumflex, `7` horn, `8` breve, `9` = đ.
    Vni,
}

/// Where the tone mark goes on `oa`, `oe`, `uy` without a final consonant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToneStyle {
    /// `hòa`, `thủy`
    #[default]
    Traditional,
    /// `hoà`, `thuỷ`
    Modern,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Letter {
    /// Lowercase base letter (or a literal digit in VNI).
    pub base: char,
    pub mark: Mark,
    pub upper: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Word {
    pub letters: Vec<Letter>,
    pub tone: Tone,
    /// Set after a double-key undo (`ass`, `aaa`): later keys are literal.
    frozen: bool,
    /// The undo cancelled a mark on what was valid Vietnamese (`carr` →
    /// `car`, not `cả`), so the user deliberately wants these letters.
    cancelled: bool,
}

impl Word {
    pub fn from_keys(keys: &[char], method: InputMethod) -> Word {
        let mut word = Word::default();
        for &key in keys {
            word.apply(key, method);
        }
        word.normalize();
        word
    }

    fn apply(&mut self, key: char, method: InputMethod) {
        if self.frozen {
            self.push(key);
            return;
        }
        let k = key.to_ascii_lowercase();
        match method {
            InputMethod::Telex => match k {
                's' => self.tone_key(Tone::Acute, key),
                'f' => self.tone_key(Tone::Grave, key),
                'r' => self.tone_key(Tone::Hook, key),
                'x' => self.tone_key(Tone::Tilde, key),
                'j' => self.tone_key(Tone::Dot, key),
                'z' => self.clear_tone(key),
                'a' | 'e' | 'o' => self.circumflex(&[k], key),
                'w' => self.horn(&['a', 'o', 'u'], true, key),
                'd' => self.stroke(key),
                _ => self.push(key),
            },
            InputMethod::Vni => match k {
                '1' => self.tone_key(Tone::Acute, key),
                '2' => self.tone_key(Tone::Grave, key),
                '3' => self.tone_key(Tone::Hook, key),
                '4' => self.tone_key(Tone::Tilde, key),
                '5' => self.tone_key(Tone::Dot, key),
                '0' => self.clear_tone(key),
                '6' => self.circumflex(&['a', 'e', 'o'], key),
                '7' => self.horn(&['o', 'u'], false, key),
                '8' => self.horn(&['a'], false, key),
                '9' => self.stroke(key),
                _ => self.push(key),
            },
        }
    }

    fn push(&mut self, key: char) {
        let base = chars::to_lower(key);
        self.letters.push(Letter {
            base,
            mark: Mark::None,
            upper: base != key,
        });
    }

    fn has_nucleus(&self) -> bool {
        !syllable::parse(&self.letters).nucleus.is_empty()
    }

    /// Set `mark` on all of `idx`; if they already have it, undo and type the key literally.
    fn toggle(&mut self, idx: &[usize], mark: Mark, key: char) {
        if idx.iter().all(|&i| self.letters[i].mark == mark) {
            self.freeze();
            for &i in idx {
                self.letters[i].mark = Mark::None;
            }
            self.push(key);
        } else {
            for &i in idx {
                self.letters[i].mark = mark;
            }
        }
    }

    /// Called just before a double-key undo is applied.
    fn freeze(&mut self) {
        self.cancelled = self.is_valid(Mode::Prefix);
        self.frozen = true;
    }

    pub fn cancelled(&self) -> bool {
        self.cancelled
    }

    fn tone_key(&mut self, tone: Tone, key: char) {
        if !self.has_nucleus() {
            self.push(key);
        } else if self.tone == tone {
            self.freeze();
            self.tone = Tone::Level;
            self.push(key);
        } else {
            self.tone = tone;
        }
    }

    fn clear_tone(&mut self, key: char) {
        if self.has_nucleus() && self.tone != Tone::Level {
            self.tone = Tone::Level;
        } else {
            self.push(key);
        }
    }

    /// Last vowel in the nucleus whose base is one of `bases`.
    fn find_in_nucleus(&self, bases: &[char]) -> Option<usize> {
        let nucleus = syllable::parse(&self.letters).nucleus;
        nucleus
            .clone()
            .rev()
            .find(|&i| bases.contains(&self.letters[i].base))
    }

    fn circumflex(&mut self, bases: &[char], key: char) {
        let Some(i) = self.find_in_nucleus(bases) else {
            self.push(key);
            return;
        };
        // In Telex the key is also a vowel: prefer whichever reading can still
        // be Vietnamese (`ngoeo` is ngoẹo's `oeo`, not `ôe`).
        if key.is_ascii_alphabetic() {
            let mut as_letter = self.clone();
            as_letter.push(key);
            let mut as_mark = self.clone();
            as_mark.toggle(&[i], Mark::Circumflex, key);
            if !as_mark.is_valid(Mode::Prefix) && as_letter.is_valid(Mode::Prefix) {
                *self = as_letter;
                return;
            }
        }
        self.toggle(&[i], Mark::Circumflex, key);
    }

    /// Horn on `o`/`u`, breve on `a`. A `uo` pair becomes `ươ` when letters
    /// follow and `uơ` at the end (`thuở`). In Telex a lone `w` types `ư`.
    fn horn(&mut self, bases: &[char], standalone: bool, key: char) {
        let nucleus = syllable::parse(&self.letters).nucleus;
        if bases.contains(&'o') {
            let pair = nucleus.clone().find(|&i| {
                i + 1 < nucleus.end
                    && self.letters[i].base == 'u'
                    && self.letters[i + 1].base == 'o'
            });
            if let Some(i) = pair {
                if i + 2 < self.letters.len() {
                    self.toggle(&[i, i + 1], Mark::Horn, key);
                } else {
                    self.toggle(&[i + 1], Mark::Horn, key);
                }
                return;
            }
        }
        match self.find_in_nucleus(bases) {
            Some(i) => {
                let mark = if self.letters[i].base == 'a' {
                    Mark::Breve
                } else {
                    Mark::Horn
                };
                self.toggle(&[i], mark, key);
            }
            None if standalone => self.letters.push(Letter {
                base: 'u',
                mark: Mark::Horn,
                upper: key.is_ascii_uppercase(),
            }),
            None => self.push(key),
        }
    }

    fn stroke(&mut self, key: char) {
        if self.letters.first().is_some_and(|l| l.base == 'd') {
            self.toggle(&[0], Mark::Stroke, key);
        } else {
            self.push(key);
        }
    }

    /// `ưo` or `uơ` followed by more letters is always `ươ` (`nguwoif` →
    /// `người`, `huowng` → `hương`); on its own `uơ` stays (`thuở`, `huơ`).
    fn normalize(&mut self) {
        let n = self.letters.len();
        for i in 0..n.saturating_sub(2) {
            let (u, o) = (self.letters[i], self.letters[i + 1]);
            let after_q = i > 0 && self.letters[i - 1].base == 'q';
            if u.base != 'u' || o.base != 'o' || after_q {
                continue;
            }
            match (u.mark, o.mark) {
                (Mark::Horn, Mark::None) => self.letters[i + 1].mark = Mark::Horn,
                (Mark::None, Mark::Horn) => self.letters[i].mark = Mark::Horn,
                _ => {}
            }
        }
    }

    pub fn is_valid(&self, mode: Mode) -> bool {
        syllable::is_valid(&self.letters, self.tone, mode)
    }

    pub fn render(&self, style: ToneStyle) -> String {
        let parts = syllable::parse(&self.letters);
        let tone_at = syllable::tone_position(&self.letters, &parts, style);
        self.letters
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let tone = if Some(i) == tone_at {
                    self.tone
                } else {
                    Tone::Level
                };
                let c = chars::compose(l.base, l.mark, tone).unwrap_or(l.base);
                if l.upper { chars::to_upper(c) } else { c }
            })
            .collect()
    }
}

/// Canonical keystrokes that type `text` (marks right after their letter,
/// tone last), e.g. `tiếng` → `tieengs` in Telex, `tie6ng1` in VNI.
pub fn encode(text: &str, method: InputMethod) -> Vec<char> {
    let mut keys = Vec::with_capacity(text.len() + 1);
    let mut tone = Tone::Level;
    for c in text.chars() {
        let lower = chars::to_lower(c);
        let upper = lower != c;
        let (base, mark, t) = chars::decompose(lower);
        if t != Tone::Level {
            tone = t;
        }
        let cased = |k: char| if upper { chars::to_upper(k) } else { k };
        keys.push(cased(base));
        let mark_key = match (method, mark) {
            (_, Mark::None) => None,
            (InputMethod::Telex, Mark::Circumflex) => Some(base),
            (InputMethod::Telex, Mark::Breve | Mark::Horn) => Some('w'),
            (InputMethod::Telex, Mark::Stroke) => Some('d'),
            (InputMethod::Vni, Mark::Circumflex) => Some('6'),
            (InputMethod::Vni, Mark::Horn) => Some('7'),
            (InputMethod::Vni, Mark::Breve) => Some('8'),
            (InputMethod::Vni, Mark::Stroke) => Some('9'),
        };
        keys.extend(mark_key.map(cased));
    }
    let tone_key = match (method, tone) {
        (_, Tone::Level) => None,
        (InputMethod::Telex, Tone::Acute) => Some('s'),
        (InputMethod::Telex, Tone::Grave) => Some('f'),
        (InputMethod::Telex, Tone::Hook) => Some('r'),
        (InputMethod::Telex, Tone::Tilde) => Some('x'),
        (InputMethod::Telex, Tone::Dot) => Some('j'),
        (InputMethod::Vni, Tone::Acute) => Some('1'),
        (InputMethod::Vni, Tone::Grave) => Some('2'),
        (InputMethod::Vni, Tone::Hook) => Some('3'),
        (InputMethod::Vni, Tone::Tilde) => Some('4'),
        (InputMethod::Vni, Tone::Dot) => Some('5'),
    };
    keys.extend(tone_key);
    keys
}
