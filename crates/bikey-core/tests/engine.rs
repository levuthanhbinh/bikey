use bikey_core::{Config, Engine, InputMethod, MAX_KEYS, ToneStyle, encode};

fn engine(method: InputMethod, tone_style: ToneStyle) -> Engine {
    Engine::new(Config {
        method,
        tone_style,
        ..Config::default()
    })
}

/// Type `keys` as one word and commit it.
fn type_word(e: &mut Engine, keys: &str) -> String {
    for c in keys.chars() {
        assert!(e.key(c).is_some(), "key {c:?} rejected in {keys:?}");
    }
    e.commit()
}

fn telex(keys: &str) -> String {
    type_word(&mut Engine::default(), keys)
}

fn vni(keys: &str) -> String {
    type_word(&mut engine(InputMethod::Vni, ToneStyle::Traditional), keys)
}

#[test]
fn telex_vietnamese() {
    let cases = [
        ("tieengs", "tiếng"),
        ("tiengse", "tiếng"), // marks can come after the final
        ("vieetj", "việt"),
        ("nguwowif", "người"),
        ("nguwoif", "người"), // ưo + more letters becomes ươ
        ("dduwowcj", "được"),
        ("hoaf", "hòa"),
        ("quyeets", "quyết"),
        ("giuwowngf", "giường"),
        ("gif", "gì"),
        ("ginf", "gìn"),
        ("khuyur", "khuỷu"),
        ("tooi", "tôi"),
        ("ddaay", "đây"),
        ("ddi", "đi"),
        ("dd", "đ"),
        ("muwa", "mưa"),
        ("trawng", "trăng"),
        ("thuowr", "thuở"),
        ("hoawcj", "hoặc"),
        ("tuaanf", "tuần"),
        ("asf", "à"), // later tone replaces earlier one
        ("asz", "a"), // z clears the tone
        ("chuyeenj", "chuyện"),
        ("nghieeng", "nghiêng"),
        ("anh", "anh"),
    ];
    for (keys, want) in cases {
        assert_eq!(telex(keys), want, "telex {keys}");
    }
}

#[test]
fn w_alone_is_never_u_horn() {
    for word in ["w", "W", "mwa", "wa", "tw", "ww", "new"] {
        assert_eq!(telex(word), word, "telex {word}");
    }
    // It still marks a vowel.
    assert_eq!(telex("muwa"), "mưa");
    assert_eq!(telex("trawng"), "trăng");
    assert_eq!(telex("sawps"), "sắp");
    assert_eq!(telex("Sawps"), "Sắp");
    assert_eq!(telex("sapws"), "sắp"); // the mark can come after the final
}

#[test]
fn case_is_preserved() {
    assert_eq!(telex("VIEETJ"), "VIỆT");
    assert_eq!(telex("Vieetj"), "Việt");
    assert_eq!(telex("DDaay"), "Đây");
    assert_eq!(telex("Nguwowif"), "Người");
}

#[test]
fn tone_styles() {
    let modern = |k| type_word(&mut engine(InputMethod::Telex, ToneStyle::Modern), k);
    assert_eq!(telex("hoaf"), "hòa");
    assert_eq!(modern("hoaf"), "hoà");
    assert_eq!(telex("thuyr"), "thủy");
    assert_eq!(modern("thuyr"), "thuỷ");
    // With a final consonant both styles agree.
    assert_eq!(telex("toans"), "toán");
    assert_eq!(modern("toans"), "toán");
}

#[test]
fn english_stays_english() {
    for word in [
        "text", "class", "hello", "window", "address", "the", "with", "keyboard", "yes", "Text",
        "CLASS", "mess", "boss", "taxi", "user", "where", "new",
    ] {
        assert_eq!(telex(word), word, "telex {word}");
    }
}

#[test]
fn preedit_while_typing() {
    let mut e = Engine::default();
    let shown: Vec<String> = "tieengs"
        .chars()
        .map(|c| e.key(c).unwrap().preedit)
        .collect();
    assert_eq!(shown, ["t", "ti", "tie", "tiê", "tiên", "tiêng", "tiếng"]);
    // English is shown as typed once it can no longer be Vietnamese.
    let mut e = Engine::default();
    let shown: Vec<String> = "text".chars().map(|c| e.key(c).unwrap().preedit).collect();
    assert_eq!(shown.last().unwrap(), "text");
}

#[test]
fn incomplete_syllable_commits_as_typed() {
    // `yes` reads as `yé` mid-word, but `yé` is not a complete syllable.
    let mut e = Engine::default();
    for c in "yes".chars() {
        e.key(c);
    }
    assert_eq!(e.commit(), "yes");
}

#[test]
fn double_key_types_the_letter() {
    // Doubling a mark key cancels it and types the letter, so either side of
    // an ambiguous word stays reachable.
    assert_eq!(telex("hass"), "has");
    assert_eq!(telex("aaa"), "aa");
    assert_eq!(telex("carr"), "car");
    assert_eq!(telex("ddd"), "dd");
    // Cancelling inside something that was never Vietnamese keeps the keys.
    assert_eq!(telex("class"), "class");
}

#[test]
fn vni_vietnamese() {
    let cases = [
        ("tie6ng1", "tiếng"),
        ("Vie65t", "Việt"),
        ("d9u7o7c5", "được"),
        ("nguo72i", "người"),
        ("hoa2", "hòa"),
        ("tra8ng", "trăng"),
        ("a10", "a"),
    ];
    for (keys, want) in cases {
        assert_eq!(vni(keys), want, "vni {keys}");
    }
}

#[test]
fn digits_outside_words() {
    let mut e = engine(InputMethod::Vni, ToneStyle::Traditional);
    assert!(
        e.key('2').is_none(),
        "VNI digit with no word must pass through"
    );
    let mut e = Engine::default();
    e.key('a');
    assert!(e.key('1').is_none(), "Telex never takes digits");
}

#[test]
fn auto_english_off_always_converts() {
    let mut e = Engine::new(Config {
        auto_english: false,
        ..Config::default()
    });
    assert_eq!(type_word(&mut e, "text"), "tẽt");
}

#[test]
fn user_english_words() {
    let mut e = Engine::default();
    assert_eq!(type_word(&mut e, "bas"), "bá");
    assert!(e.add_english_word("Bas"));
    assert_eq!(type_word(&mut e, "bas"), "bas");
    assert!(!e.add_english_word(""));
    assert!(!e.add_english_word("two words"));
    assert!(!e.add_english_word(&"a".repeat(MAX_KEYS + 1)));
}

#[test]
fn backspace() {
    let mut e = Engine::default();
    for c in "toans".chars() {
        e.key(c);
    }
    assert_eq!(e.preedit(), "toán");
    // Removing the final moves the tone back: tóa (traditional).
    assert_eq!(e.backspace().unwrap().preedit, "tóa");
    assert_eq!(e.backspace().unwrap().preedit, "tó");
    assert_eq!(e.backspace().unwrap().preedit, "t");
    assert_eq!(e.backspace().unwrap().preedit, "");
    assert!(
        e.backspace().is_none(),
        "empty buffer passes Backspace through"
    );

    let mut e = Engine::default();
    for c in "text".chars() {
        e.key(c);
    }
    assert_eq!(e.backspace().unwrap().preedit, "tex");
}

#[test]
fn long_input_is_committed_in_chunks() {
    let mut e = Engine::default();
    let mut committed = String::new();
    for _ in 0..(MAX_KEYS * 2 + 5) {
        committed.push_str(&e.key('b').unwrap().commit);
    }
    committed.push_str(&e.commit());
    assert_eq!(committed, "b".repeat(MAX_KEYS * 2 + 5));
}

#[test]
fn reset_and_commit_clear_state() {
    let mut e = Engine::default();
    e.key('a');
    e.reset();
    assert!(!e.is_composing());
    assert_eq!(e.commit(), "");
}

#[test]
fn debug_never_shows_typed_text() {
    let mut e = Engine::default();
    for c in "secret".chars() {
        e.key(c);
    }
    let dbg = format!("{e:?}");
    assert!(!dbg.contains("secret") && !dbg.contains("ecret"), "{dbg}");
}

#[test]
fn encode_examples() {
    assert_eq!(encode("tiếng", InputMethod::Telex), "tieengs");
    assert_eq!(encode("Việt", InputMethod::Vni), "Vie6t5");
    assert_eq!(encode("được", InputMethod::Telex), "dduwowcj");
}

/// Random keys, backspaces and commits must never panic, and output stays bounded NFC text.
#[test]
fn random_input_never_panics() {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let keys: Vec<char> = "abcdefghijklmnopqrstuvwxyzADEOUW0123456789"
        .chars()
        .collect();
    for method in [InputMethod::Telex, InputMethod::Vni] {
        for style in [ToneStyle::Traditional, ToneStyle::Modern] {
            let mut e = engine(method, style);
            for _ in 0..200_000 {
                let r = next();
                match r % 20 {
                    0 => {
                        let _ = e.backspace();
                    }
                    1 => {
                        let text = e.commit();
                        assert!(text.chars().count() <= MAX_KEYS * 2);
                    }
                    _ => {
                        let c = keys[(r >> 8) as usize % keys.len()];
                        if let Some(out) = e.key(c) {
                            assert!(out.preedit.chars().count() <= MAX_KEYS * 2);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn mixed_sentence() {
    let mut e = Engine::default();
    let typed = "Hoom nay tooi dayj lowps IELTS, these students have a test veef Writing Task 2 \
                 nhuwng hoj vaanx chuwa hieeur the question";
    let mut out = Vec::new();
    for word in typed.split(' ') {
        let mut text = String::new();
        for c in word.chars() {
            if let Some(o) = e.key(c) {
                text.push_str(&o.commit);
            } else {
                text.push_str(&e.commit());
                text.push(c);
            }
        }
        text.push_str(&e.commit());
        out.push(text);
    }
    assert_eq!(
        out.join(" "),
        "Hôm nay tôi dạy lớp IELTS, these students have a test về Writing Task 2 \
         nhưng họ vẫn chưa hiểu the question"
    );
}
