//! Drives the engine with arbitrary keys, backspaces, commits and config
//! changes. Any panic, or output that breaks the engine's bounds, is a bug.

#![no_main]

use bikey_core::{Config, Engine, InputMethod, MAX_KEYS, ToneStyle};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut engine = Engine::default();
    for &b in data {
        match b {
            0 => {
                let _ = engine.backspace();
            }
            1 => {
                let text = engine.commit();
                assert!(text.chars().count() <= MAX_KEYS * 2);
            }
            2 => engine.reset(),
            3..=6 => engine.set_config(Config {
                method: if b & 1 == 0 { InputMethod::Telex } else { InputMethod::Vni },
                tone_style: if b & 2 == 0 { ToneStyle::Traditional } else { ToneStyle::Modern },
                ..Config::default()
            }),
            _ => {
                if let Some(out) = engine.key(char::from(b)) {
                    assert!(out.preedit.chars().count() <= MAX_KEYS * 2);
                }
            }
        }
    }
    let _ = engine.commit();
});
