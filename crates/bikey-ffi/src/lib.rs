//! C ABI over `bikey-core`, declared in `include/bikey.h`.
//!
//! Every entry point checks its pointers, never lets a panic unwind into the
//! host application, and bounds every input length before reading it.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use bikey_core::{Engine, InputMethod, MAX_KEYS, Output, ToneStyle};

pub const BIKEY_OK: i32 = 0;
pub const BIKEY_HANDLED: i32 = 1;
pub const BIKEY_NOT_HANDLED: i32 = 0;
pub const BIKEY_ERR_NULL: i32 = -1;
pub const BIKEY_ERR_INVALID: i32 = -2;
pub const BIKEY_ERR_PANIC: i32 = -3;

pub const BIKEY_METHOD_TELEX: u32 = 0;
pub const BIKEY_METHOD_VNI: u32 = 1;
pub const BIKEY_TONE_TRADITIONAL: u32 = 0;
pub const BIKEY_TONE_MODERN: u32 = 1;

/// Opaque handle: the engine plus the buffers `BikeyOutput` points into.
pub struct BikeyEngine {
    engine: Engine,
    commit: String,
    preedit: String,
}

// Hand-written so the output buffers (typed text) never reach a log.
impl std::fmt::Debug for BikeyEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BikeyEngine")
            .field("engine", &self.engine)
            .finish_non_exhaustive()
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BikeyOutput {
    pub commit: *const u8,
    pub commit_len: usize,
    pub preedit: *const u8,
    pub preedit_len: usize,
}

impl BikeyOutput {
    const EMPTY: BikeyOutput = BikeyOutput {
        commit: ptr::null(),
        commit_len: 0,
        preedit: ptr::null(),
        preedit_len: 0,
    };
}

impl BikeyEngine {
    fn store(&mut self, output: Option<Output>) -> i32 {
        if let Some(o) = output {
            self.commit = o.commit;
            self.preedit = o.preedit;
            BIKEY_HANDLED
        } else {
            self.clear();
            BIKEY_NOT_HANDLED
        }
    }

    fn clear(&mut self) {
        self.commit.clear();
        self.preedit.clear();
    }

    fn output(&self) -> BikeyOutput {
        BikeyOutput {
            commit: self.commit.as_ptr(),
            commit_len: self.commit.len(),
            preedit: self.preedit.as_ptr(),
            preedit_len: self.preedit.len(),
        }
    }
}

/// Run `f` with panics contained. After a panic the word is dropped so the
/// engine is usable again.
///
/// # Safety
/// `engine` is null or a live pointer from [`bikey_new`] that nothing else is using.
unsafe fn with_engine(engine: *mut BikeyEngine, f: impl FnOnce(&mut BikeyEngine) -> i32) -> i32 {
    // SAFETY: guaranteed by the caller (see above).
    let Some(e) = (unsafe { engine.as_mut() }) else {
        return BIKEY_ERR_NULL;
    };
    if let Ok(code) = catch_unwind(AssertUnwindSafe(|| f(&mut *e))) {
        code
    } else {
        e.engine.reset();
        e.clear();
        BIKEY_ERR_PANIC
    }
}

/// Like [`with_engine`] for calls that fill a `BikeyOutput`: `out` is emptied
/// first and filled only when the call returns [`BIKEY_HANDLED`].
///
/// # Safety
/// As [`with_engine`]; `out` is null or valid for one `BikeyOutput` write.
unsafe fn with_output(
    engine: *mut BikeyEngine,
    out: *mut BikeyOutput,
    f: impl FnOnce(&mut BikeyEngine) -> i32,
) -> i32 {
    if out.is_null() {
        return BIKEY_ERR_NULL;
    }
    // SAFETY: `out` is non-null and valid for writes per the caller.
    unsafe { out.write(BikeyOutput::EMPTY) };
    // SAFETY: forwarded caller guarantee.
    unsafe {
        with_engine(engine, |e| {
            let code = f(e);
            if code == BIKEY_HANDLED {
                // SAFETY: as above.
                out.write(e.output());
            }
            code
        })
    }
}

fn with_config(e: &mut BikeyEngine, change: impl FnOnce(&mut bikey_core::Config)) -> i32 {
    let mut config = e.engine.config().clone();
    change(&mut config);
    e.engine.set_config(config);
    BIKEY_OK
}

/// Create an engine with default settings (Telex, traditional tones, smart English).
#[unsafe(no_mangle)]
pub extern "C" fn bikey_new() -> *mut BikeyEngine {
    catch_unwind(|| {
        Box::into_raw(Box::new(BikeyEngine {
            engine: Engine::default(),
            commit: String::new(),
            preedit: String::new(),
        }))
    })
    .unwrap_or(ptr::null_mut())
}

/// # Safety
/// `engine` is null or a pointer from [`bikey_new`] that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bikey_free(engine: *mut BikeyEngine) {
    if !engine.is_null() {
        // SAFETY: allocated by `bikey_new` via `Box::into_raw` and not yet freed.
        drop(unsafe { Box::from_raw(engine) });
    }
}

/// # Safety
/// `engine` is null or a live pointer from [`bikey_new`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bikey_set_method(engine: *mut BikeyEngine, method: u32) -> i32 {
    let method = match method {
        BIKEY_METHOD_TELEX => InputMethod::Telex,
        BIKEY_METHOD_VNI => InputMethod::Vni,
        _ => return BIKEY_ERR_INVALID,
    };
    // SAFETY: forwarded caller guarantee.
    unsafe { with_engine(engine, |e| with_config(e, |c| c.method = method)) }
}

/// # Safety
/// `engine` is null or a live pointer from [`bikey_new`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bikey_set_tone_style(engine: *mut BikeyEngine, style: u32) -> i32 {
    let style = match style {
        BIKEY_TONE_TRADITIONAL => ToneStyle::Traditional,
        BIKEY_TONE_MODERN => ToneStyle::Modern,
        _ => return BIKEY_ERR_INVALID,
    };
    // SAFETY: forwarded caller guarantee.
    unsafe { with_engine(engine, |e| with_config(e, |c| c.tone_style = style)) }
}

/// # Safety
/// `engine` is null or a live pointer from [`bikey_new`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bikey_set_auto_english(engine: *mut BikeyEngine, enabled: bool) -> i32 {
    // SAFETY: forwarded caller guarantee.
    unsafe { with_engine(engine, |e| with_config(e, |c| c.auto_english = enabled)) }
}

/// # Safety
/// `engine` is null or a live pointer from [`bikey_new`]; `word` is null or
/// valid for reads of `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bikey_add_english_word(
    engine: *mut BikeyEngine,
    word: *const u8,
    len: usize,
) -> i32 {
    if word.is_null() {
        return BIKEY_ERR_NULL;
    }
    // Checked before reading, so a bogus length never becomes a huge slice.
    if len == 0 || len > MAX_KEYS {
        return BIKEY_ERR_INVALID;
    }
    // SAFETY: `word` is non-null and valid for `len` (≤ MAX_KEYS) bytes per the caller.
    let bytes = unsafe { std::slice::from_raw_parts(word, len) };
    let Ok(word) = std::str::from_utf8(bytes) else {
        return BIKEY_ERR_INVALID;
    };
    // SAFETY: forwarded caller guarantee.
    unsafe {
        with_engine(engine, |e| {
            if e.engine.add_english_word(word) {
                BIKEY_OK
            } else {
                BIKEY_ERR_INVALID
            }
        })
    }
}

/// # Safety
/// `engine` is null or a live pointer from [`bikey_new`]; `out` is null or
/// valid for writing one `BikeyOutput`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bikey_key(
    engine: *mut BikeyEngine,
    codepoint: u32,
    out: *mut BikeyOutput,
) -> i32 {
    let c = char::from_u32(codepoint);
    // SAFETY: forwarded caller guarantee.
    unsafe {
        with_output(engine, out, |e| match c {
            Some(c) => {
                let output = e.engine.key(c);
                e.store(output)
            }
            None => BIKEY_ERR_INVALID,
        })
    }
}

/// # Safety
/// As [`bikey_key`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bikey_backspace(engine: *mut BikeyEngine, out: *mut BikeyOutput) -> i32 {
    // SAFETY: forwarded caller guarantee.
    unsafe {
        with_output(engine, out, |e| {
            let output = e.engine.backspace();
            e.store(output)
        })
    }
}

/// # Safety
/// As [`bikey_key`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bikey_commit(engine: *mut BikeyEngine, out: *mut BikeyOutput) -> i32 {
    // SAFETY: forwarded caller guarantee.
    unsafe {
        with_output(engine, out, |e| {
            let commit = e.engine.commit();
            e.store(Some(Output {
                commit,
                preedit: String::new(),
            }))
        })
    }
}

/// # Safety
/// `engine` is null or a live pointer from [`bikey_new`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bikey_reset(engine: *mut BikeyEngine) {
    // SAFETY: forwarded caller guarantee.
    unsafe {
        with_engine(engine, |e| {
            e.engine.reset();
            e.clear();
            BIKEY_OK
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(p: *const u8, len: usize) -> String {
        if len == 0 {
            return String::new();
        }
        // SAFETY: the engine guarantees `p` points to `len` bytes of UTF-8
        // until the next call, and the test reads it before that.
        String::from_utf8(unsafe { std::slice::from_raw_parts(p, len) }.to_vec()).unwrap()
    }

    fn type_word(e: *mut BikeyEngine, keys: &str) -> String {
        let mut out = BikeyOutput::EMPTY;
        for c in keys.chars() {
            // SAFETY: `e` is live, `out` is a valid local.
            assert_eq!(
                unsafe { bikey_key(e, c as u32, &raw mut out) },
                BIKEY_HANDLED
            );
        }
        // SAFETY: as above.
        assert_eq!(unsafe { bikey_commit(e, &raw mut out) }, BIKEY_HANDLED);
        text(out.commit, out.commit_len)
    }

    #[test]
    fn typing_through_the_c_api() {
        let e = bikey_new();
        assert!(!e.is_null());
        assert_eq!(type_word(e, "tieengs"), "tiếng");
        assert_eq!(type_word(e, "class"), "class");

        let mut out = BikeyOutput::EMPTY;
        // SAFETY: `e` is live, `out` is a valid local.
        unsafe {
            assert_eq!(bikey_key(e, 'v' as u32, &raw mut out), BIKEY_HANDLED);
            assert_eq!(text(out.preedit, out.preedit_len), "v");
            assert_eq!(bikey_key(e, ' ' as u32, &raw mut out), BIKEY_NOT_HANDLED);
            assert_eq!(out.commit_len + out.preedit_len, 0);
            assert_eq!(bikey_backspace(e, &raw mut out), BIKEY_HANDLED);
            assert_eq!(bikey_backspace(e, &raw mut out), BIKEY_NOT_HANDLED);

            assert_eq!(bikey_set_method(e, BIKEY_METHOD_VNI), BIKEY_OK);
            assert_eq!(bikey_set_tone_style(e, BIKEY_TONE_MODERN), BIKEY_OK);
        }
        assert_eq!(type_word(e, "hoa2"), "hoà");
        // SAFETY: allocated by `bikey_new` above and not used afterwards.
        unsafe { bikey_free(e) };
    }

    #[test]
    fn rejects_bad_arguments() {
        let e = bikey_new();
        let mut out = BikeyOutput::EMPTY;
        let word = b"bas";
        let not_utf8 = [0xffu8, 0xfe];
        // SAFETY: null and dangling-free arguments only; `e` is live.
        unsafe {
            assert_eq!(
                bikey_key(ptr::null_mut(), 'a' as u32, &raw mut out),
                BIKEY_ERR_NULL
            );
            assert_eq!(bikey_key(e, 'a' as u32, ptr::null_mut()), BIKEY_ERR_NULL);
            assert_eq!(bikey_key(e, 0xD800, &raw mut out), BIKEY_ERR_INVALID); // surrogate
            assert_eq!(bikey_key(e, 0x11_0000, &raw mut out), BIKEY_ERR_INVALID);
            assert_eq!(bikey_set_method(e, 7), BIKEY_ERR_INVALID);
            assert_eq!(bikey_set_tone_style(ptr::null_mut(), 0), BIKEY_ERR_NULL);
            assert_eq!(bikey_add_english_word(e, ptr::null(), 3), BIKEY_ERR_NULL);
            assert_eq!(
                bikey_add_english_word(e, word.as_ptr(), 0),
                BIKEY_ERR_INVALID
            );
            // Oversized lengths are refused before anything is read.
            assert_eq!(
                bikey_add_english_word(e, word.as_ptr(), usize::MAX),
                BIKEY_ERR_INVALID
            );
            assert_eq!(
                bikey_add_english_word(e, not_utf8.as_ptr(), 2),
                BIKEY_ERR_INVALID
            );
            assert_eq!(
                bikey_add_english_word(e, word.as_ptr(), word.len()),
                BIKEY_OK
            );
            bikey_reset(ptr::null_mut());
            bikey_free(ptr::null_mut());
        }
        assert_eq!(type_word(e, "bas"), "bas");
        // SAFETY: allocated by `bikey_new` above and not used afterwards.
        unsafe { bikey_free(e) };
    }
}
