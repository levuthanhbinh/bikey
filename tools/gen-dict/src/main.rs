//! Usage: `cargo run -p gen-dict -- WORDLIST...`
//!
//! Types every word of each list through the engine (Telex, built-in
//! exceptions off) and prints the ones that would come out as Vietnamese:
//!
//! ```text
//! word<TAB>vietnamese<TAB>natural|unnatural
//! ```
//!
//! `natural` means a Vietnamese typist would press exactly these keys for that
//! Vietnamese word (`cos` → `có`), so adding it to english.txt costs
//! Vietnamese users; review those by hand. `unnatural` ones (`these` → `thế`)
//! are safe to add.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::process::ExitCode;

use bikey_core::{Config, Engine, InputMethod, MAX_KEYS, encode};

fn main() -> ExitCode {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: gen-dict WORDLIST...");
        return ExitCode::FAILURE;
    }
    let mut engine = Engine::new(Config {
        english_exceptions: false,
        ..Config::default()
    });
    let mut found = BTreeMap::new();
    for path in &paths {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("{path}: {e}");
                return ExitCode::FAILURE;
            }
        };
        for word in text.split_whitespace() {
            let word = word.trim_end_matches(['%', '!', '&', '+', '^', '=', '#', '<', '~']);
            if word.is_empty()
                || word.len() > MAX_KEYS
                || !word.bytes().all(|b| b.is_ascii_lowercase())
            {
                continue;
            }
            for c in word.chars() {
                engine.key(c);
            }
            let out = engine.commit();
            if out != word {
                let natural = encode(&out, InputMethod::Telex) == word;
                found.insert(word.to_owned(), (out, natural));
            }
        }
    }
    for (word, (vi, natural)) in &found {
        let kind = if *natural { "natural" } else { "unnatural" };
        println!("{word}\t{vi}\t{kind}");
    }
    ExitCode::SUCCESS
}
