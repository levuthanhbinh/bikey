//! Vietnamese syllable structure: initial + vowel nucleus + final, plus tone rules.

use core::ops::Range;

use crate::chars::{self, Mark, Tone};
use crate::word::{Letter, ToneStyle};

const INITIALS: &[&str] = &[
    "b", "c", "ch", "d", "đ", "g", "gh", "gi", "h", "k", "kh", "l", "m", "n", "ng", "ngh", "nh",
    "p", "ph", "qu", "r", "s", "t", "th", "tr", "v", "x",
];

const FINALS: &[&str] = &["c", "ch", "m", "n", "ng", "nh", "p", "t"];

/// Whether a nucleus may, may not, or must be followed by a final consonant.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FinalRule {
    Open,
    Optional,
    Required,
}

const NUCLEI: &[(&str, FinalRule)] = &[
    ("a", FinalRule::Optional),
    ("ă", FinalRule::Required),
    ("â", FinalRule::Required),
    ("e", FinalRule::Optional),
    ("ê", FinalRule::Optional),
    ("i", FinalRule::Optional),
    ("o", FinalRule::Optional),
    ("ô", FinalRule::Optional),
    ("ơ", FinalRule::Optional),
    ("u", FinalRule::Optional),
    ("ư", FinalRule::Optional),
    ("y", FinalRule::Optional),
    ("ai", FinalRule::Open),
    ("ao", FinalRule::Open),
    ("au", FinalRule::Open),
    ("ay", FinalRule::Open),
    ("âu", FinalRule::Open),
    ("ây", FinalRule::Open),
    ("eo", FinalRule::Open),
    ("êu", FinalRule::Open),
    ("ia", FinalRule::Open),
    ("iê", FinalRule::Required),
    ("iu", FinalRule::Open),
    ("oa", FinalRule::Optional),
    ("oă", FinalRule::Required),
    ("oe", FinalRule::Optional),
    ("oi", FinalRule::Open),
    ("oo", FinalRule::Required),
    ("ôi", FinalRule::Open),
    ("ơi", FinalRule::Open),
    ("ua", FinalRule::Open),
    ("uâ", FinalRule::Required),
    ("uê", FinalRule::Optional),
    ("ui", FinalRule::Open),
    ("uô", FinalRule::Required),
    ("uơ", FinalRule::Open),
    ("uy", FinalRule::Optional),
    ("ưa", FinalRule::Open),
    ("ưi", FinalRule::Open),
    ("ưu", FinalRule::Open),
    ("ươ", FinalRule::Required),
    ("yê", FinalRule::Required),
    ("iêu", FinalRule::Open),
    ("oai", FinalRule::Open),
    ("oay", FinalRule::Open),
    ("oeo", FinalRule::Open),
    ("uây", FinalRule::Open),
    ("uôi", FinalRule::Open),
    ("ươi", FinalRule::Open),
    ("ươu", FinalRule::Open),
    ("uya", FinalRule::Open),
    ("uyê", FinalRule::Required),
    ("uyu", FinalRule::Open),
    ("yêu", FinalRule::Open),
];

/// Validate a finished syllable, or one that could still become valid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// While typing: the letters so far can be extended (more letters or marks) into a syllable.
    Prefix,
    /// At commit: the letters form a complete syllable.
    Complete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Parts {
    pub initial: Range<usize>,
    pub nucleus: Range<usize>,
    pub fin: Range<usize>,
}

/// Split letters into initial consonants, vowel nucleus and the rest. The `u`
/// of `qu` and the `i` of `gi` (before another vowel) belong to the initial.
pub(crate) fn parse(letters: &[Letter]) -> Parts {
    let n = letters.len();
    let mut i = 0;
    while i < n && !chars::is_vowel(letters[i].base) {
        i += 1;
    }
    if i < n {
        let qu = i >= 1
            && letters[i - 1].base == 'q'
            && letters[i].base == 'u'
            && letters[i].mark == Mark::None;
        let gi = i == 1
            && letters[0].base == 'g'
            && letters[i].base == 'i'
            && i + 1 < n
            && chars::is_vowel(letters[i + 1].base);
        if qu || gi {
            i += 1;
        }
    }
    let start = i;
    while i < n && chars::is_vowel(letters[i].base) {
        i += 1;
    }
    Parts {
        initial: 0..start,
        nucleus: start..i,
        fin: i..n,
    }
}

fn consonants(letters: &[Letter]) -> String {
    letters
        .iter()
        .map(|l| if l.mark == Mark::Stroke { 'đ' } else { l.base })
        .collect()
}

fn nucleus_ok(nucleus: &[Letter], has_final: bool, mode: Mode) -> bool {
    NUCLEI.iter().any(|&(entry, rule)| {
        let target = entry.chars().map(|c| {
            let (base, mark, _) = chars::decompose(c);
            (base, mark)
        });
        let len = entry.chars().count();
        match mode {
            Mode::Complete => {
                let exact = len == nucleus.len()
                    && nucleus
                        .iter()
                        .zip(target)
                        .all(|(l, (b, m))| l.base == b && l.mark == m);
                exact
                    && match rule {
                        FinalRule::Open => !has_final,
                        FinalRule::Optional => true,
                        FinalRule::Required => has_final,
                    }
            }
            Mode::Prefix => {
                // A plain vowel may still receive its mark later (`tie` → `tiê`).
                let relaxed = |(l, (b, m)): (&Letter, (char, Mark))| {
                    l.base == b && (l.mark == m || l.mark == Mark::None)
                };
                if has_final {
                    len == nucleus.len()
                        && rule != FinalRule::Open
                        && nucleus.iter().zip(target).all(relaxed)
                } else {
                    len >= nucleus.len() && nucleus.iter().zip(target).all(relaxed)
                }
            }
        }
    })
}

/// Spelling rules tying the initial to the first vowel (`k`/`c`, `gh`/`g`, `ngh`/`ng`).
fn spelling_ok(initial: &str, first_vowel: char) -> bool {
    let front = matches!(first_vowel, 'e' | 'i' | 'y');
    match initial {
        "c" | "ng" => !front,
        "k" => front,
        "gh" | "ngh" => matches!(first_vowel, 'e' | 'i'),
        "g" => first_vowel != 'e',
        _ => true,
    }
}

pub(crate) fn is_valid(letters: &[Letter], tone: Tone, mode: Mode) -> bool {
    if !letters.iter().all(|l| l.base.is_ascii_lowercase()) {
        return false;
    }
    let parts = parse(letters);
    let initial = consonants(&letters[parts.initial.clone()]);
    let nucleus = &letters[parts.nucleus.clone()];
    let fin = &letters[parts.fin.clone()];

    if nucleus.is_empty() {
        return match mode {
            Mode::Prefix => initial.is_empty() || INITIALS.iter().any(|i| i.starts_with(&initial)),
            // A lone `đ` is the common abbreviation for đồng.
            Mode::Complete => initial == "đ",
        };
    }
    if !initial.is_empty() && !INITIALS.contains(&initial.as_str()) {
        return false;
    }
    if !spelling_ok(&initial, nucleus[0].base) {
        return false;
    }
    if fin
        .iter()
        .any(|l| chars::is_vowel(l.base) || l.mark != Mark::None)
    {
        return false;
    }
    let fin = consonants(fin);
    let fin_ok = match mode {
        Mode::Prefix => fin.is_empty() || FINALS.iter().any(|f| f.starts_with(&fin)),
        Mode::Complete => fin.is_empty() || FINALS.contains(&fin.as_str()),
    };
    if !fin_ok || !nucleus_ok(nucleus, !fin.is_empty(), mode) {
        return false;
    }
    if fin == "ch" || fin == "nh" {
        let last = nucleus[nucleus.len() - 1];
        let ok = match last.base {
            'a' => last.mark == Mark::None,
            'e' => {
                last.mark == Mark::Circumflex || (mode == Mode::Prefix && last.mark == Mark::None)
            }
            'i' | 'y' => true,
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    // Stop finals only take sắc or nặng (`tết` is fine, `tẽt` is not Vietnamese).
    if matches!(fin.as_str(), "c" | "ch" | "p" | "t") {
        let ok = match tone {
            Tone::Acute | Tone::Dot => true,
            Tone::Level => mode == Mode::Prefix,
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    true
}

/// Index of the letter that carries the tone mark.
pub(crate) fn tone_position(letters: &[Letter], parts: &Parts, style: ToneStyle) -> Option<usize> {
    let start = parts.nucleus.start;
    let nucleus = &letters[parts.nucleus.clone()];
    if nucleus.is_empty() {
        return None;
    }
    // A modified vowel takes the tone; in `ươ` it is the `ơ`.
    if let Some(k) = nucleus
        .iter()
        .rposition(|l| matches!(l.mark, Mark::Circumflex | Mark::Breve | Mark::Horn))
    {
        return Some(start + k);
    }
    let has_final = !parts.fin.is_empty();
    Some(match nucleus.len() {
        1 => start,
        2 => {
            let modern_pair = matches!(
                (nucleus[0].base, nucleus[1].base),
                ('o', 'a' | 'e') | ('u', 'y')
            );
            if has_final || (style == ToneStyle::Modern && modern_pair) {
                start + 1
            } else {
                start
            }
        }
        _ => start + 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word::{InputMethod, Word, encode};

    fn letters(s: &str) -> Vec<Letter> {
        s.chars()
            .map(|c| {
                let (base, mark, _) = chars::decompose(c);
                Letter {
                    base,
                    mark,
                    upper: false,
                }
            })
            .collect()
    }

    /// Every valid syllable built from the tables, rendered with every tone.
    fn all_syllables(style: ToneStyle) -> Vec<String> {
        let tones = [
            Tone::Level,
            Tone::Acute,
            Tone::Grave,
            Tone::Hook,
            Tone::Tilde,
            Tone::Dot,
        ];
        let mut out = Vec::new();
        for initial in INITIALS.iter().chain([&""]) {
            for (nucleus, _) in NUCLEI {
                for fin in FINALS.iter().chain([&""]) {
                    for tone in tones {
                        let mut word = Word::default();
                        word.letters = letters(&format!("{initial}{nucleus}{fin}"));
                        word.tone = tone;
                        // `gi`/`qu` + i/u-initial nuclei re-parse differently; skip those.
                        if parse(&word.letters).initial.len() != initial.chars().count() {
                            continue;
                        }
                        if word.is_valid(Mode::Complete) {
                            out.push(word.render(style));
                        }
                    }
                }
            }
        }
        out
    }

    #[test]
    fn generator_finds_real_words() {
        let all = all_syllables(ToneStyle::Traditional);
        for w in [
            "tiếng",
            "người",
            "được",
            "hòa",
            "quyết",
            "giường",
            "khuỷu",
            "ăn",
            "thuở",
        ] {
            assert!(all.iter().any(|s| s == w), "{w} not generated");
        }
        for w in ["tẽt", "ce", "ka", "ngi", "ghá", "yê", "ăt"] {
            assert!(!all.iter().any(|s| s == w), "{w} wrongly generated");
        }
    }

    #[test]
    fn encode_then_type_round_trips() {
        for style in [ToneStyle::Traditional, ToneStyle::Modern] {
            for s in all_syllables(style) {
                let bases: String = s.chars().map(|c| chars::decompose(c).0).collect();
                if bases.contains("oo") {
                    continue; // `xoong`/`boóc`: Telex `oo` is ô by design
                }
                for method in [InputMethod::Telex, InputMethod::Vni] {
                    let keys = encode(&s, method);
                    let typed = Word::from_keys(&keys, method);
                    assert_eq!(typed.render(style), s, "{method:?} keys {keys:?}");
                    assert!(typed.is_valid(Mode::Complete), "{s} invalid after typing");
                }
            }
        }
    }
}
