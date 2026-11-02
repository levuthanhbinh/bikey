//! The TSF text service: key handling and the composition (underlined word).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use bikey_core::{Config, Engine, InputMethod, Output, ToneStyle};
use windows::Win32::Foundation::{E_UNEXPECTED, LPARAM, WPARAM};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardLayout, GetKeyboardState, INPUT, INPUT_0, INPUT_KEYBOARD,
    KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, SendInput, ToUnicodeEx, VIRTUAL_KEY,
    VK_BACK, VK_CAPITAL, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU,
    VK_NUMLOCK, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SCROLL, VK_SHIFT,
};
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, GUID_PROP_ATTRIBUTE, IEnumTfDisplayAttributeInfo, ITfCategoryMgr,
    ITfDisplayAttributeInfo, ITfDisplayAttributeProvider, ITfDisplayAttributeProvider_Impl,
    ITfRange,
};
use windows::Win32::UI::TextServices::{
    GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, ITfComposition, ITfCompositionSink,
    ITfCompositionSink_Impl, ITfContext, ITfContextComposition, ITfDocumentMgr,
    ITfInsertAtSelection, ITfKeyEventSink, ITfKeyEventSink_Impl, ITfKeystrokeMgr, ITfSource,
    ITfTextInputProcessor, ITfTextInputProcessor_Impl, ITfTextInputProcessorEx,
    ITfTextInputProcessorEx_Impl, ITfThreadMgr, ITfThreadMgrEventSink, ITfThreadMgrEventSink_Impl,
    TF_ANCHOR_END, TF_ES_READWRITE, TF_GRAVITY_BACKWARD, TF_GRAVITY_FORWARD, TF_IAS_QUERYONLY,
    TF_MOD_ALT, TF_PRESERVEDKEY,
};
use windows_core::{BOOL, GUID, IUnknown, IUnknownImpl, Interface, Ref, Result, implement, w};

use crate::display::{AttributeEnum, GUID_ATTR_INPUT, InputAttribute};
use crate::tsf::{self, wide};
use crate::{DllRef, GUID_TOGGLE_KEY, guard};

/// What a key press means for the current word.
enum Action {
    /// A letter (or VNI digit) that goes into the word.
    Word(char),
    /// Backspace inside the word.
    Backspace,
    /// A typed character bikey inserts itself: one that ends a word (space,
    /// punctuation), inserted with the word because in classic controls a committed
    /// word reaches the app after a passed-through key; or a key the Vietnamese
    /// layout would mistype.
    CommitWith(char),
    /// Any other key while composing: finish the word, then let the key through.
    Commit,
    /// Not ours.
    Ignore,
}

#[derive(Default)]
struct State {
    engine: RefCell<Engine>,
    thread_mgr: RefCell<Option<ITfThreadMgr>>,
    client_id: Cell<u32>,
    /// The open composition and the context it lives in.
    composition: RefCell<Option<(ITfContext, ITfComposition)>>,
    /// Our own `ITfCompositionSink`, held only between Activate and Deactivate.
    sink: RefCell<Option<ITfCompositionSink>>,
    thread_sink_cookie: Cell<Option<u32>>,
    /// TSF atom for the "being typed" display attribute (0 = none).
    input_atom: Cell<u32>,
}

#[implement(
    ITfTextInputProcessorEx,
    ITfTextInputProcessor,
    ITfKeyEventSink,
    ITfCompositionSink,
    ITfThreadMgrEventSink,
    ITfDisplayAttributeProvider
)]
pub struct TextService {
    state: Rc<State>,
    _dll: DllRef,
}

// Hand-written: never print engine state (it holds the word being typed).
impl std::fmt::Debug for TextService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextService")
            .field("composing", &self.state.composing())
            .finish_non_exhaustive()
    }
}

impl TextService {
    pub fn create() -> IUnknown {
        TextService {
            state: Rc::default(),
            _dll: DllRef::new(),
        }
        .into()
    }
}

/// Settings from `HKCU\Software\bikey` (DWORDs `Method`, `ToneStyle`,
/// `AutoEnglish`). Missing or unreadable values (e.g. in sandboxed apps) keep
/// the defaults.
fn load_config() -> Config {
    let read = |name: windows_core::PCWSTR| -> Option<u32> {
        let mut value = 0u32;
        let mut size = u32::try_from(size_of::<u32>()).ok()?;
        // SAFETY: `value`/`size` describe a valid 4-byte buffer.
        unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!("Software\\bikey"),
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
    };
    let mut config = Config::default();
    if read(w!("Method")) == Some(1) {
        config.method = InputMethod::Vni;
    }
    if read(w!("ToneStyle")) == Some(1) {
        config.tone_style = ToneStyle::Modern;
    }
    if read(w!("AutoEnglish")) == Some(0) {
        config.auto_english = false;
    }
    config
}

fn key_down(vk: u16) -> bool {
    // SAFETY: trivial Win32 call.
    unsafe { GetKeyState(i32::from(vk)) < 0 }
}

/// Whether Windows' Vietnamese keyboard layout is active. Its number row and some
/// punctuation keys type Vietnamese letters, which breaks digits and VNI.
fn vietnamese_layout() -> bool {
    // SAFETY: trivial Win32 call.
    let hkl = unsafe { GetKeyboardLayout(0) }.0 as usize;
    (hkl >> 16) & 0xffff == 0x042A
}

/// What a number-row or punctuation key types on a US keyboard.
fn us_key(vk: u32, shift: bool) -> Option<char> {
    let (plain, shifted) = match vk {
        0x30 => ('0', ')'),
        0x31 => ('1', '!'),
        0x32 => ('2', '@'),
        0x33 => ('3', '#'),
        0x34 => ('4', '$'),
        0x35 => ('5', '%'),
        0x36 => ('6', '^'),
        0x37 => ('7', '&'),
        0x38 => ('8', '*'),
        0x39 => ('9', '('),
        0xBA => (';', ':'),
        0xBB => ('=', '+'),
        0xBC => (',', '<'),
        0xBD => ('-', '_'),
        0xBE => ('.', '>'),
        0xBF => ('/', '?'),
        0xC0 => ('`', '~'),
        0xDB => ('[', '{'),
        0xDC => ('\\', '|'),
        0xDD => (']', '}'),
        0xDE => ('\'', '"'),
        _ => return None,
    };
    Some(if shift { shifted } else { plain })
}

/// The character this key produces with the current layout and modifiers,
/// without disturbing dead-key state.
fn key_char(vk: u32, lparam: LPARAM) -> Option<char> {
    let mut keys = [0u8; 256];
    // SAFETY: valid 256-byte buffer.
    unsafe { GetKeyboardState(&mut keys) }.ok()?;
    if vietnamese_layout() {
        let shift = keys[usize::from(VK_SHIFT.0)] & 0x80 != 0;
        if let Some(c) = us_key(vk, shift) {
            return Some(c);
        }
    }
    let scan = u32::try_from((lparam.0 >> 16) & 0xff).ok()?;
    let mut buf = [0u16; 4];
    // SAFETY: valid buffers; flag 0x4 leaves the keyboard (dead-key) state untouched.
    let n = unsafe { ToUnicodeEx(vk, scan, &keys, &mut buf, 0x4, Some(GetKeyboardLayout(0))) };
    if n == 1 {
        char::from_u32(u32::from(buf[0]))
    } else {
        None
    }
}

/// Send the key the user just pressed once more, for keys bikey cannot insert
/// itself (Enter, arrows, shortcuts). A committed word reaches classic controls
/// after a passed-through key, and a claimed key is not handed back, so this is
/// the only way to keep the order. The word is already finished, so the re-sent
/// key is classified `Ignore` and goes straight to the app. Never used for text.
fn resend_key(wparam: WPARAM, lparam: LPARAM) {
    let Ok(vk) = u16::try_from(wparam.0) else {
        return;
    };
    let scan = u16::try_from((lparam.0 >> 16) & 0xff).unwrap_or(0);
    let extended = (lparam.0 >> 24) & 1 == 1;
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: scan,
                dwFlags: if extended {
                    KEYEVENTF_EXTENDEDKEY
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let size = i32::try_from(size_of::<INPUT>()).unwrap_or(0);
    // SAFETY: one well-formed keyboard input.
    unsafe { SendInput(&[input], size) };
}

fn is_modifier(vk: u16) -> bool {
    [
        VK_SHIFT,
        VK_LSHIFT,
        VK_RSHIFT,
        VK_CONTROL,
        VK_LCONTROL,
        VK_RCONTROL,
        VK_MENU,
        VK_LMENU,
        VK_RMENU,
        VK_LWIN,
        VK_RWIN,
        VK_CAPITAL,
        VK_NUMLOCK,
        VK_SCROLL,
    ]
    .iter()
    .any(|k| k.0 == vk)
}

impl State {
    fn composing(&self) -> bool {
        self.composition.borrow().is_some()
    }

    fn enabled(&self) -> bool {
        let tm = self.thread_mgr.borrow();
        tm.as_ref().is_some_and(|tm| {
            tsf::compartment_i32(tm, &GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) != Some(0)
        })
    }

    fn classify(&self, ctx: &ITfContext, wparam: WPARAM, lparam: LPARAM) -> Action {
        let action = self.classify_inner(ctx, wparam, lparam);
        trace!(
            "classify vk={:#x} -> {}",
            wparam.0,
            match action {
                Action::Word(_) => "word",
                Action::Backspace => "backspace",
                Action::CommitWith(_) => "commit+char",
                Action::Commit => "commit",
                Action::Ignore => "ignore",
            }
        );
        action
    }

    fn classify_inner(&self, ctx: &ITfContext, wparam: WPARAM, lparam: LPARAM) -> Action {
        let pass = if self.composing() {
            Action::Commit
        } else {
            Action::Ignore
        };
        let Ok(vk) = u16::try_from(wparam.0) else {
            return pass;
        };
        if is_modifier(vk) {
            return Action::Ignore;
        }
        if [VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
            .iter()
            .any(|k| key_down(k.0))
        {
            return pass;
        }
        let action = if vk == VK_BACK.0 {
            if self.composing() {
                Action::Backspace
            } else {
                Action::Ignore
            }
        } else {
            match key_char(u32::from(vk), lparam) {
                Some(c) if self.engine.borrow().accepts(c) => Action::Word(c),
                Some(c) if self.composing() && !c.is_control() => Action::CommitWith(c),
                // Under the Vietnamese layout the app would type the wrong character.
                Some(c) if vietnamese_layout() && us_key(u32::from(vk), false).is_some() => {
                    Action::CommitWith(c)
                }
                _ => return pass,
            }
        };
        // Only now pay for the field checks, and only for keys we would take.
        trace!(
            "enabled={} secure={}",
            self.enabled(),
            tsf::is_secure_field(ctx, self.client_id.get())
        );
        if !self.enabled() || tsf::is_secure_field(ctx, self.client_id.get()) {
            return pass;
        }
        action
    }

    /// Show `out` in the document. Runs inside a read/write edit session.
    fn apply_in_session(&self, ctx: &ITfContext, ec: u32, out: &Output) -> Result<()> {
        if !out.commit.is_empty() {
            self.set_text(ctx, ec, &out.commit)?;
            self.end_composition(ec)?;
        }
        if !out.preedit.is_empty() {
            self.set_text(ctx, ec, &out.preedit)?;
        } else if self.composing() {
            self.set_text(ctx, ec, "")?;
            self.end_composition(ec)?;
        }
        Ok(())
    }

    /// Replace the composition text (starting a composition at the caret if
    /// needed) and put the caret after it.
    fn set_text(&self, ctx: &ITfContext, ec: u32, text: &str) -> Result<()> {
        let existing = self.composition.borrow().as_ref().map(|(_, c)| c.clone());
        let composition = if let Some(c) = existing {
            c
        } else {
            let insert: ITfInsertAtSelection = ctx.cast()?;
            let contexts: ITfContextComposition = ctx.cast()?;
            let sink = self.sink.borrow().clone().ok_or(E_UNEXPECTED)?;
            // SAFETY: valid edit cookie and live interfaces.
            let c = unsafe {
                let range = insert.InsertTextAtSelection(ec, TF_IAS_QUERYONLY, &[])?;
                contexts.StartComposition(ec, &range, &sink)?
            };
            trace!("started composition");
            *self.composition.borrow_mut() = Some((ctx.clone(), c.clone()));
            c
        };
        // SAFETY: valid edit cookie and live interfaces.
        unsafe {
            let range = composition.GetRange()?;
            // An empty range does not grow when text is inserted at its
            // position, so let the end follow the insertion, then make the
            // composition span exactly the new text. Otherwise TSF sees an
            // empty composition, ends it, and the caret stays in front.
            range.SetGravity(ec, TF_GRAVITY_BACKWARD, TF_GRAVITY_FORWARD)?;
            range.SetText(ec, 0, &wide(text))?;
            composition.ShiftEnd(ec, &range)?;
            composition.ShiftStart(ec, &range)?;
            #[cfg(feature = "trace")]
            {
                let mut buf = [0u16; 64];
                let mut n = 0u32;
                let _ = composition.GetRange()?.GetText(ec, 0, &mut buf, &raw mut n);
                trace!(
                    "set_text {text:?}: composition now {:?}",
                    String::from_utf16_lossy(&buf[..n as usize])
                );
            }
            self.mark_input(ctx, ec, &range);
            let caret = range.Clone()?;
            caret.Collapse(ec, TF_ANCHOR_END)?;
            tsf::set_selection(ctx, ec, &caret)
        }
    }

    /// Tag `range` with the "being typed" display attribute (best effort).
    fn mark_input(&self, ctx: &ITfContext, ec: u32, range: &ITfRange) {
        let atom = self.input_atom.get();
        if atom == 0 {
            return;
        }
        let value = tsf::variant_i32(i32::try_from(atom).unwrap_or(0));
        // SAFETY: valid edit cookie; `value` is a plain integer VARIANT.
        let _ = unsafe {
            ctx.GetProperty(&GUID_PROP_ATTRIBUTE)
                .and_then(|p| p.SetValue(ec, range, &raw const value))
        };
    }

    fn end_composition(&self, ec: u32) -> Result<()> {
        let taken = self.composition.borrow_mut().take();
        match taken {
            // SAFETY: valid edit cookie; clearing our attribute is best effort.
            Some((ctx, c)) => unsafe {
                if let (Ok(property), Ok(range)) =
                    (ctx.GetProperty(&GUID_PROP_ATTRIBUTE), c.GetRange())
                {
                    let _ = property.Clear(ec, &range);
                }
                c.EndComposition(ec)
            },
            None => Ok(()),
        }
    }

    /// Finish the current word: write its final text (English fix-ups happen
    /// here) and end the composition.
    fn commit(self: &Rc<Self>) {
        let Some((ctx, _)) = self.composition.borrow().clone() else {
            self.engine.borrow_mut().reset();
            return;
        };
        let text = self.engine.borrow_mut().commit();
        trace!("commit {text:?}");
        let state = self.clone();
        let ctx2 = ctx.clone();
        let done = tsf::edit(&ctx, self.client_id.get(), TF_ES_READWRITE, move |ec| {
            state.set_text(&ctx2, ec, &text)?;
            state.end_composition(ec)
        });
        if done.is_err() {
            // The text already on screen stays; just forget the composition.
            self.composition.borrow_mut().take();
        }
    }

    /// Finish the word with `c` appended. Returns false if that failed, in
    /// which case the word stays on screen and the key should pass through.
    fn commit_with(self: &Rc<Self>, current: &ITfContext, c: char) -> bool {
        let existing = self
            .composition
            .borrow()
            .as_ref()
            .map(|(ctx, _)| ctx.clone());
        let ctx = existing.unwrap_or_else(|| current.clone());
        let mut text = self.engine.borrow_mut().commit();
        text.push(c);
        trace!("commit+char {text:?}");
        let state = self.clone();
        let ctx2 = ctx.clone();
        let done = tsf::edit(&ctx, self.client_id.get(), TF_ES_READWRITE, move |ec| {
            state.set_text(&ctx2, ec, &text)?;
            state.end_composition(ec)
        });
        if done.is_err() {
            self.composition.borrow_mut().take();
        }
        done.is_ok()
    }

    /// Feed `out` to the document; on failure drop the word so the key can pass through.
    fn apply(self: &Rc<Self>, ctx: &ITfContext, out: Output) -> bool {
        let state = self.clone();
        let ctx2 = ctx.clone();
        let done = tsf::edit(ctx, self.client_id.get(), TF_ES_READWRITE, move |ec| {
            state.apply_in_session(&ctx2, ec, &out)
        });
        trace!("apply -> {done:?}");
        if done.is_err() {
            self.engine.borrow_mut().reset();
            self.composition.borrow_mut().take();
        }
        done.is_ok()
    }

    fn deactivate(&self) {
        if let Some((ctx, _)) = self.composition.borrow().clone() {
            let text = self.engine.borrow_mut().commit();
            let composition = self.composition.borrow_mut().take();
            let _ = tsf::edit(&ctx, self.client_id.get(), TF_ES_READWRITE, move |ec| {
                if let Some((_, c)) = composition {
                    // SAFETY: valid edit cookie.
                    unsafe {
                        c.GetRange()?.SetText(ec, 0, &wide(&text))?;
                        c.EndComposition(ec)?;
                    }
                }
                Ok(())
            });
        }
        self.engine.borrow_mut().reset();
        self.composition.borrow_mut().take();
        let tm = self.thread_mgr.borrow_mut().take();
        if let Some(tm) = tm {
            let id = self.client_id.get();
            if let Ok(keys) = tm.cast::<ITfKeystrokeMgr>() {
                let toggle = toggle_key();
                // SAFETY: plain COM calls; failures just mean it was not registered.
                unsafe {
                    let _ = keys.UnpreserveKey(&GUID_TOGGLE_KEY, &raw const toggle);
                    let _ = keys.UnadviseKeyEventSink(id);
                }
            }
            if let (Some(cookie), Ok(source)) =
                (self.thread_sink_cookie.take(), tm.cast::<ITfSource>())
            {
                // SAFETY: cookie came from AdviseSink on this source.
                let _ = unsafe { source.UnadviseSink(cookie) };
            }
        }
        // Breaks the self-reference taken in Activate.
        self.sink.borrow_mut().take();
        self.client_id.set(0);
    }
}

fn toggle_key() -> TF_PRESERVEDKEY {
    TF_PRESERVEDKEY {
        uVKey: u32::from(b'Z'),
        uModifiers: TF_MOD_ALT,
    }
}

impl ITfTextInputProcessor_Impl for TextService_Impl {
    fn Activate(&self, ptim: Ref<ITfThreadMgr>, tid: u32) -> Result<()> {
        ITfTextInputProcessorEx_Impl::ActivateEx(self, ptim, tid, 0)
    }

    fn Deactivate(&self) -> Result<()> {
        guard(|| {
            self.state.deactivate();
            Ok(())
        })
    }
}

impl ITfTextInputProcessorEx_Impl for TextService_Impl {
    fn ActivateEx(&self, ptim: Ref<ITfThreadMgr>, tid: u32, _flags: u32) -> Result<()> {
        let activated = guard(|| {
            let tm = ptim.ok()?.clone();
            let state = &self.state;
            state.client_id.set(tid);
            *state.thread_mgr.borrow_mut() = Some(tm.clone());
            state.engine.borrow_mut().set_config(load_config());
            *state.sink.borrow_mut() = Some(self.to_interface::<ITfCompositionSink>());

            let keys: ITfKeystrokeMgr = tm.cast()?;
            let key_sink: ITfKeyEventSink = self.to_interface();
            let toggle = toggle_key();
            // SAFETY: plain COM calls with live interfaces.
            unsafe {
                keys.AdviseKeyEventSink(tid, &key_sink, true)?;
                keys.PreserveKey(
                    tid,
                    &GUID_TOGGLE_KEY,
                    &raw const toggle,
                    &wide("Bật/tắt tiếng Việt"),
                )?;
            }
            let source: ITfSource = tm.cast()?;
            let thread_sink: ITfThreadMgrEventSink = self.to_interface();
            // SAFETY: plain COM call with live interfaces.
            let cookie = unsafe { source.AdviseSink(&ITfThreadMgrEventSink::IID, &thread_sink)? };
            state.thread_sink_cookie.set(Some(cookie));
            // SAFETY: plain COM calls; without the atom words just aren't underlined.
            let atom = unsafe {
                CoCreateInstance::<_, ITfCategoryMgr>(
                    &CLSID_TF_CategoryMgr,
                    None,
                    CLSCTX_INPROC_SERVER,
                )
                .and_then(|m| m.RegisterGUID(&GUID_ATTR_INPUT))
            };
            state.input_atom.set(atom.unwrap_or(0));
            // Vietnamese on when bikey is selected.
            tsf::set_compartment_i32(&tm, &GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, tid, 1)?;
            Ok(())
        });
        if activated.is_err() {
            let _ = guard(|| {
                self.state.deactivate();
                Ok(())
            });
        }
        activated
    }
}

impl ITfKeyEventSink_Impl for TextService_Impl {
    fn OnSetFocus(&self, _foreground: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnTestKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        guard(|| {
            let ctx = pic.ok()?;
            Ok(match self.state.classify(ctx, wparam, lparam) {
                // `Commit` is claimed too: OnKeyDown finishes the word, then sends
                // the key again so the app sees it after the word.
                Action::Word(_) | Action::Backspace | Action::CommitWith(_) | Action::Commit => {
                    true.into()
                }
                Action::Ignore => false.into(),
            })
        })
    }

    fn OnKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        guard(|| {
            let ctx = pic.ok()?;
            let state = &self.state;
            let eaten = match state.classify(ctx, wparam, lparam) {
                Action::Word(c) => {
                    let out = state.engine.borrow_mut().key(c);
                    out.is_some_and(|out| state.apply(ctx, out))
                }
                Action::Backspace => {
                    let out = state.engine.borrow_mut().backspace();
                    out.is_some_and(|out| state.apply(ctx, out))
                }
                Action::CommitWith(c) => state.commit_with(ctx, c),
                Action::Commit => {
                    state.commit();
                    resend_key(wparam, lparam);
                    true
                }
                Action::Ignore => false,
            };
            Ok(eaten.into())
        })
    }

    fn OnTestKeyUp(&self, _pic: Ref<ITfContext>, _w: WPARAM, _l: LPARAM) -> Result<BOOL> {
        Ok(false.into())
    }

    fn OnKeyUp(&self, _pic: Ref<ITfContext>, _w: WPARAM, _l: LPARAM) -> Result<BOOL> {
        Ok(false.into())
    }

    fn OnPreservedKey(&self, _pic: Ref<ITfContext>, rguid: *const GUID) -> Result<BOOL> {
        guard(|| {
            // SAFETY: TSF passes a valid GUID pointer (checked for null).
            if rguid.is_null() || unsafe { *rguid } != GUID_TOGGLE_KEY {
                return Ok(false.into());
            }
            let state = &self.state;
            state.commit();
            let tm = state.thread_mgr.borrow().clone();
            if let Some(tm) = tm {
                let on = state.enabled();
                tsf::set_compartment_i32(
                    &tm,
                    &GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,
                    state.client_id.get(),
                    i32::from(!on),
                )?;
            }
            Ok(true.into())
        })
    }
}

impl ITfCompositionSink_Impl for TextService_Impl {
    fn OnCompositionTerminated(&self, ec: u32, composition: Ref<ITfComposition>) -> Result<()> {
        guard(|| {
            // The app ended the word (e.g. a click elsewhere): apply the final
            // text with the cookie we were given, then forget the word.
            let state = &self.state;
            let text = state.engine.borrow_mut().commit();
            trace!("composition terminated by app; final {text:?}");
            if let Ok(c) = composition.ok() {
                // SAFETY: `ec` is the write cookie TSF passed for this callback.
                let _ = unsafe { c.GetRange().and_then(|r| r.SetText(ec, 0, &wide(&text))) };
            }
            state.composition.borrow_mut().take();
            Ok(())
        })
    }
}

impl ITfThreadMgrEventSink_Impl for TextService_Impl {
    fn OnInitDocumentMgr(&self, _dim: Ref<ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }

    fn OnUninitDocumentMgr(&self, _dim: Ref<ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }

    fn OnSetFocus(
        &self,
        _focus: Ref<ITfDocumentMgr>,
        _previous: Ref<ITfDocumentMgr>,
    ) -> Result<()> {
        // Focus moved to another field: finish the word where it was typed.
        trace!("document focus changed");
        guard(|| {
            self.state.commit();
            Ok(())
        })
    }

    fn OnPushContext(&self, _pic: Ref<ITfContext>) -> Result<()> {
        Ok(())
    }

    fn OnPopContext(&self, _pic: Ref<ITfContext>) -> Result<()> {
        Ok(())
    }
}

impl ITfDisplayAttributeProvider_Impl for TextService_Impl {
    fn EnumDisplayAttributeInfo(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(AttributeEnum::default().into())
    }

    fn GetDisplayAttributeInfo(&self, guid: *const GUID) -> Result<ITfDisplayAttributeInfo> {
        // SAFETY: checked for null; TSF passes a valid GUID.
        if !guid.is_null() && unsafe { *guid } == GUID_ATTR_INPUT {
            Ok(InputAttribute.into())
        } else {
            Err(windows::Win32::Foundation::E_INVALIDARG.into())
        }
    }
}
