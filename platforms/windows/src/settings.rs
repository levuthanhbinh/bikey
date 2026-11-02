//! User settings in `HKCU\Software\bikey`, shared by every app bikey runs in.
//!
//! All values are DWORDs. Missing or unreadable values (e.g. in sandboxed apps)
//! read as the defaults, and writes that fail are ignored.

use bikey_core::{Config, InputMethod, ToneStyle};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_DWORD, RRF_RT_REG_DWORD, RegGetValueW, RegSetKeyValueW,
};
use windows_core::{PCWSTR, w};

const KEY: PCWSTR = w!("Software\\bikey");
const METHOD: PCWSTR = w!("Method");
const TONE_STYLE: PCWSTR = w!("ToneStyle");
const AUTO_ENGLISH: PCWSTR = w!("AutoEnglish");
const ENABLED: PCWSTR = w!("Enabled");

fn read(name: PCWSTR) -> Option<u32> {
    let mut value = 0u32;
    let mut size = u32::try_from(size_of::<u32>()).ok()?;
    // SAFETY: `value`/`size` describe a valid 4-byte buffer.
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            KEY,
            name,
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&raw mut size),
        )
    }
    .ok()
    .ok()?;
    Some(value)
}

fn write(name: PCWSTR, value: u32) {
    let size = u32::try_from(size_of::<u32>()).unwrap_or(4);
    // SAFETY: `value` is a valid 4-byte buffer; this creates our own key if needed.
    let _ = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            KEY,
            name,
            REG_DWORD.0,
            Some((&raw const value).cast()),
            size,
        )
    };
}

pub fn config() -> Config {
    let mut config = Config::default();
    if read(METHOD) == Some(1) {
        config.method = InputMethod::Vni;
    }
    if read(TONE_STYLE) == Some(1) {
        config.tone_style = ToneStyle::Modern;
    }
    if read(AUTO_ENGLISH) == Some(0) {
        config.auto_english = false;
    }
    config
}

pub fn enabled() -> bool {
    read(ENABLED) != Some(0)
}

pub fn set_enabled(on: bool) {
    write(ENABLED, u32::from(on));
}

pub fn set_method(method: InputMethod) {
    write(METHOD, u32::from(method == InputMethod::Vni));
}

pub fn set_tone_style(style: ToneStyle) {
    write(TONE_STYLE, u32::from(style == ToneStyle::Modern));
}
