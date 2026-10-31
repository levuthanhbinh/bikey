//! Precomposed (NFC) Vietnamese character tables.

/// Vowel modifier, or the stroke on `đ`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mark {
    None,
    /// `â ê ô`
    Circumflex,
    /// `ă`
    Breve,
    /// `ơ ư`
    Horn,
    /// `đ`
    Stroke,
}

/// The six Vietnamese tones (thanh).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tone {
    /// ngang (no mark)
    #[default]
    Level,
    /// sắc
    Acute,
    /// huyền
    Grave,
    /// hỏi
    Hook,
    /// ngã
    Tilde,
    /// nặng
    Dot,
}

impl Tone {
    const ALL: [Tone; 6] = [
        Tone::Level,
        Tone::Acute,
        Tone::Grave,
        Tone::Hook,
        Tone::Tilde,
        Tone::Dot,
    ];

    fn index(self) -> usize {
        match self {
            Tone::Level => 0,
            Tone::Acute => 1,
            Tone::Grave => 2,
            Tone::Hook => 3,
            Tone::Tilde => 4,
            Tone::Dot => 5,
        }
    }
}

/// Each row: base letter, mark, then the six tone forms in `Tone::ALL` order.
const VOWELS: [(char, Mark, [char; 6]); 12] = [
    ('a', Mark::None, ['a', 'á', 'à', 'ả', 'ã', 'ạ']),
    ('a', Mark::Breve, ['ă', 'ắ', 'ằ', 'ẳ', 'ẵ', 'ặ']),
    ('a', Mark::Circumflex, ['â', 'ấ', 'ầ', 'ẩ', 'ẫ', 'ậ']),
    ('e', Mark::None, ['e', 'é', 'è', 'ẻ', 'ẽ', 'ẹ']),
    ('e', Mark::Circumflex, ['ê', 'ế', 'ề', 'ể', 'ễ', 'ệ']),
    ('i', Mark::None, ['i', 'í', 'ì', 'ỉ', 'ĩ', 'ị']),
    ('o', Mark::None, ['o', 'ó', 'ò', 'ỏ', 'õ', 'ọ']),
    ('o', Mark::Circumflex, ['ô', 'ố', 'ồ', 'ổ', 'ỗ', 'ộ']),
    ('o', Mark::Horn, ['ơ', 'ớ', 'ờ', 'ở', 'ỡ', 'ợ']),
    ('u', Mark::None, ['u', 'ú', 'ù', 'ủ', 'ũ', 'ụ']),
    ('u', Mark::Horn, ['ư', 'ứ', 'ừ', 'ử', 'ữ', 'ự']),
    ('y', Mark::None, ['y', 'ý', 'ỳ', 'ỷ', 'ỹ', 'ỵ']),
];

pub fn is_vowel(base: char) -> bool {
    matches!(base, 'a' | 'e' | 'i' | 'o' | 'u' | 'y')
}

/// Compose a lowercase base letter with a mark and tone. Returns `None` for
/// combinations Vietnamese does not have (e.g. `e` + horn).
pub fn compose(base: char, mark: Mark, tone: Tone) -> Option<char> {
    if base == 'd' {
        return match mark {
            Mark::None => Some('d'),
            Mark::Stroke => Some('đ'),
            _ => None,
        };
    }
    if !is_vowel(base) {
        return (mark == Mark::None).then_some(base);
    }
    VOWELS
        .iter()
        .find(|(b, m, _)| *b == base && *m == mark)
        .map(|(_, _, forms)| forms[tone.index()])
}

/// Split a lowercase character into (base, mark, tone). Non-Vietnamese
/// characters come back unchanged with no mark or tone.
pub fn decompose(c: char) -> (char, Mark, Tone) {
    if c == 'đ' {
        return ('d', Mark::Stroke, Tone::Level);
    }
    for (base, mark, forms) in &VOWELS {
        if let Some(i) = forms.iter().position(|f| *f == c) {
            return (*base, *mark, Tone::ALL[i]);
        }
    }
    (c, Mark::None, Tone::Level)
}

/// Lowercase/uppercase for the single-char mappings used here.
pub fn to_lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

pub fn to_upper(c: char) -> char {
    c.to_uppercase().next().unwrap_or(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_decompose_round_trip() {
        for (base, mark, forms) in &VOWELS {
            for (i, f) in forms.iter().enumerate() {
                assert_eq!(compose(*base, *mark, Tone::ALL[i]), Some(*f));
                assert_eq!(decompose(*f), (*base, *mark, Tone::ALL[i]));
                // Every form has a single-char uppercase that maps back.
                assert_eq!(to_lower(to_upper(*f)), *f);
            }
        }
        assert_eq!(compose('d', Mark::Stroke, Tone::Level), Some('đ'));
        assert_eq!(decompose('đ'), ('d', Mark::Stroke, Tone::Level));
        assert_eq!(compose('e', Mark::Horn, Tone::Level), None);
    }
}
