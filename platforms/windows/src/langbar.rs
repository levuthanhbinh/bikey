//! The input-mode button near the clock: click to turn Vietnamese on or off,
//! menu for Telex, VNI and tone style. Choices are saved in the user's settings,
//! so they apply in every app from the next word.

use std::cell::RefCell;
use std::rc::Rc;

use bikey_core::{InputMethod, ToneStyle};
use windows::Win32::Foundation::{E_INVALIDARG, E_NOINTERFACE, POINT, RECT};
use windows::Win32::Graphics::Gdi::HBITMAP;
use windows::Win32::UI::TextServices::{
    GUID_LBI_INPUTMODE, ITfLangBarItem, ITfLangBarItem_Impl, ITfLangBarItemButton,
    ITfLangBarItemButton_Impl, ITfLangBarItemSink, ITfMenu, ITfSource, ITfSource_Impl,
    TF_LANGBARITEMINFO, TF_LBI_CLK_LEFT, TF_LBI_ICON, TF_LBI_STYLE_BTN_BUTTON,
    TF_LBI_STYLE_BTN_MENU, TF_LBI_STYLE_SHOWNINTRAY, TF_LBI_TEXT, TF_LBMENUF_RADIOCHECKED,
    TfLBIClick,
};
use windows::Win32::UI::WindowsAndMessaging::{HICON, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW};
use windows_core::{BOOL, BSTR, GUID, IUnknown, Interface, PCWSTR, Ref, Result, implement};

use crate::tsf::wide;
use crate::{CLSID_BIKEY, guard, module, settings};

pub const MENU_TELEX: u32 = 1;
pub const MENU_VNI: u32 = 2;
pub const MENU_OFF: u32 = 3;
pub const MENU_TONE_TRADITIONAL: u32 = 4;
pub const MENU_TONE_MODERN: u32 = 5;

const SINK_COOKIE: u32 = 0x6269_6b65;

/// Runs after a setting changes, so the service can drop the word in progress.
type OnChange = Rc<dyn Fn()>;

#[implement(ITfLangBarItemButton, ITfLangBarItem, ITfSource)]
pub struct ModeButton {
    sink: RefCell<Option<ITfLangBarItemSink>>,
    on_change: OnChange,
}

impl std::fmt::Debug for ModeButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModeButton").finish_non_exhaustive()
    }
}

impl ModeButton {
    pub fn create(on_change: OnChange) -> ITfLangBarItemButton {
        ModeButton {
            sink: RefCell::new(None),
            on_change,
        }
        .into()
    }
}

/// Turn Vietnamese on or off, exactly as clicking the button does (Alt+Z).
pub fn toggle(button: &ITfLangBarItemButton) -> Result<()> {
    // SAFETY: plain COM call; our `OnClick` ignores the position and area.
    unsafe { button.OnClick(TF_LBI_CLK_LEFT, POINT::default(), core::ptr::null()) }
}

impl ModeButton_Impl {
    fn changed(&self) {
        (self.on_change)();
        let sink = self.sink.borrow().clone();
        if let Some(sink) = sink {
            // SAFETY: plain COM call on the sink TSF gave us.
            let _ = unsafe { sink.OnUpdate(TF_LBI_ICON | TF_LBI_TEXT) };
        }
    }
}

impl ITfLangBarItem_Impl for ModeButton_Impl {
    fn GetInfo(&self, info: *mut TF_LANGBARITEMINFO) -> Result<()> {
        if info.is_null() {
            return Err(E_INVALIDARG.into());
        }
        let mut description = [0u16; 32];
        for (slot, unit) in description.iter_mut().zip(wide("bikey")) {
            *slot = unit;
        }
        // SAFETY: non-null out pointer supplied by TSF.
        unsafe {
            info.write(TF_LANGBARITEMINFO {
                clsidService: CLSID_BIKEY,
                guidItem: GUID_LBI_INPUTMODE,
                dwStyle: TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_BTN_MENU | TF_LBI_STYLE_SHOWNINTRAY,
                ulSort: 0,
                szDescription: description,
            });
        }
        Ok(())
    }

    fn GetStatus(&self) -> Result<u32> {
        Ok(0)
    }

    fn Show(&self, _show: BOOL) -> Result<()> {
        Ok(())
    }

    fn GetTooltipString(&self) -> Result<BSTR> {
        Ok(BSTR::from(mode_text()))
    }
}

fn mode_text() -> String {
    if settings::enabled() {
        match settings::config().method {
            InputMethod::Telex => "bikey: Telex".into(),
            InputMethod::Vni => "bikey: VNI".into(),
        }
    } else {
        "bikey: English".into()
    }
}

impl ITfLangBarItemButton_Impl for ModeButton_Impl {
    fn OnClick(&self, click: TfLBIClick, _pt: &POINT, _area: *const RECT) -> Result<()> {
        guard(|| {
            if click == TF_LBI_CLK_LEFT {
                settings::set_enabled(!settings::enabled());
                self.changed();
            }
            Ok(())
        })
    }

    fn InitMenu(&self, menu: Ref<ITfMenu>) -> Result<()> {
        guard(|| {
            let menu = menu.ok()?;
            let config = settings::config();
            let on = settings::enabled();
            let check = |yes: bool| if yes { TF_LBMENUF_RADIOCHECKED } else { 0 };
            let items = [
                (
                    MENU_TELEX,
                    "Telex",
                    on && config.method == InputMethod::Telex,
                ),
                (MENU_VNI, "VNI", on && config.method == InputMethod::Vni),
                (MENU_OFF, "Tắt (English)", !on),
                (
                    MENU_TONE_TRADITIONAL,
                    "Bỏ dấu kiểu cũ (hòa)",
                    config.tone_style == ToneStyle::Traditional,
                ),
                (
                    MENU_TONE_MODERN,
                    "Bỏ dấu kiểu mới (hoà)",
                    config.tone_style == ToneStyle::Modern,
                ),
            ];
            for (id, label, checked) in items {
                // SAFETY: plain COM call; no bitmaps and no submenu.
                unsafe {
                    menu.AddMenuItem(
                        id,
                        check(checked),
                        HBITMAP::default(),
                        HBITMAP::default(),
                        &wide(label),
                        core::ptr::null_mut(),
                    )?;
                }
            }
            Ok(())
        })
    }

    fn OnMenuSelect(&self, id: u32) -> Result<()> {
        guard(|| {
            match id {
                MENU_TELEX | MENU_VNI => {
                    settings::set_enabled(true);
                    settings::set_method(if id == MENU_VNI {
                        InputMethod::Vni
                    } else {
                        InputMethod::Telex
                    });
                }
                MENU_OFF => settings::set_enabled(false),
                MENU_TONE_TRADITIONAL => settings::set_tone_style(ToneStyle::Traditional),
                MENU_TONE_MODERN => settings::set_tone_style(ToneStyle::Modern),
                _ => return Err(E_INVALIDARG.into()),
            }
            self.changed();
            Ok(())
        })
    }

    fn GetIcon(&self) -> Result<HICON> {
        let id: usize = if settings::enabled() { 2 } else { 3 };
        // SAFETY: loads an icon resource from this DLL. An integer id is the
        // documented form of the name; TSF owns and destroys the returned icon.
        let handle = unsafe {
            LoadImageW(
                Some(module()),
                PCWSTR(id as *const u16),
                IMAGE_ICON,
                16,
                16,
                LR_DEFAULTCOLOR,
            )
        }?;
        Ok(HICON(handle.0))
    }

    fn GetText(&self) -> Result<BSTR> {
        Ok(BSTR::from(mode_text()))
    }
}

impl ITfSource_Impl for ModeButton_Impl {
    fn AdviseSink(&self, riid: *const GUID, punk: Ref<IUnknown>) -> Result<u32> {
        // SAFETY: TSF passes a valid IID (checked for null).
        if riid.is_null() || unsafe { *riid } != ITfLangBarItemSink::IID {
            return Err(E_NOINTERFACE.into());
        }
        *self.sink.borrow_mut() = Some(punk.ok()?.cast()?);
        Ok(SINK_COOKIE)
    }

    fn UnadviseSink(&self, cookie: u32) -> Result<()> {
        if cookie != SINK_COOKIE {
            return Err(E_INVALIDARG.into());
        }
        self.sink.borrow_mut().take();
        Ok(())
    }
}
