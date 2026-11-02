//! The TSF text service: decides what each key means and types the result.
//!
//! bikey deliberately does not use TSF compositions: apps handle them too
//! differently (some end them after every key, classic controls show a floating
//! window). TSF is used to receive keys only in text fields while bikey is the
//! selected input method, and to recognise password fields. Text is typed with
//! `SendInput`: the difference between the word on screen and the new word, as
//! Backspaces plus Unicode characters.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use bikey_core::Engine;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardLayout, GetKeyboardState, INPUT, INPUT_0, INPUT_KEYBOARD,
    KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    SendInput, ToUnicodeEx, VIRTUAL_KEY, VK_BACK, VK_CAPITAL, VK_CONTROL, VK_ESCAPE, VK_LCONTROL,
    VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_NUMLOCK, VK_PACKET, VK_RCONTROL, VK_RMENU, VK_RSHIFT,
    VK_RWIN, VK_SCROLL, VK_SHIFT,
};
use windows::Win32::UI::TextServices::{
    GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, ITfContext, ITfDocumentMgr, ITfKeyEventSink,
    ITfKeyEventSink_Impl, ITfKeystrokeMgr, ITfSource, ITfTextInputProcessor,
    ITfTextInputProcessor_Impl, ITfTextInputProcessorEx, ITfTextInputProcessorEx_Impl,
    ITfThreadMgr, ITfThreadMgrEventSink, ITfThreadMgrEventSink_Impl, TF_MOD_ALT, TF_PRESERVEDKEY,
};
use windows::Win32::UI::TextServices::{ITfLangBarItemButton, ITfLangBarItemMgr};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, HHOOK, SetWindowsHookExW, UnhookWindowsHookEx, WH_MOUSE, WM_LBUTTONDOWN,
    WM_MBUTTONDOWN, WM_NCLBUTTONDOWN, WM_NCMBUTTONDOWN, WM_NCRBUTTONDOWN, WM_RBUTTONDOWN,
};
use windows_core::{BOOL, GUID, IUnknown, IUnknownImpl, Interface, Ref, Result, implement};

use crate::langbar::{self, ModeButton};
use crate::tsf::{self, wide};
use crate::{DllRef, GUID_TOGGLE_KEY, guard, settings};

/// What a key press means for the current word.
enum Action {
    /// A letter (or VNI digit) that goes into the word.
    Word(char),
    /// Backspace inside the word.
    Backspace,
    /// A typed character that ends the word (space, punctuation), or a key the
    /// Vietnamese layout would mistype. Typed by bikey after the word.
    CommitWith(char),
    /// Esc on a converted word, or right after one: put back the exact keys.
    Restore,
    /// Any other key inside a word (Enter, arrows): finish the word, then send
    /// the key again so the app receives it afterwards.
    Commit,
    /// Not ours.
    Ignore,
}

/// Where the current word is, in document offsets, when the app reports them.
#[derive(Clone, Copy)]
struct Span {
    start: i32,
    end: i32,
}

/// The word just finished, kept only until the next key or click so that Esc
/// can put back what was typed.
struct Finished {
    word: String,
    typed: String,
    ended_by: char,
}

#[derive(Default)]
struct State {
    engine: RefCell<Engine>,
    thread_mgr: RefCell<Option<ITfThreadMgr>>,
    client_id: Cell<u32>,
    /// The current word as it is on screen. Cleared when the word ends.
    shown: RefCell<String>,
    finished: RefCell<Option<Finished>>,
    span: Cell<Option<Span>>,
    thread_sink_cookie: Cell<Option<u32>>,
    mouse_hook: Cell<Option<HHOOK>>,
    button: RefCell<Option<ITfLangBarItemButton>>,
}

thread_local! {
    /// The service on this thread, for the mouse hook callback.
    static CURRENT: RefCell<Weak<State>> = const { RefCell::new(Weak::new()) };
}

/// A click can move the caret, and many apps do not report that through TSF, so
/// a correction would delete the wrong text. Any mouse press ends the word.
///
/// This hook covers this thread only (the app's own UI thread), not the system,
/// and reads nothing but the kind of message.
extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let press = u32::try_from(wparam.0).is_ok_and(|m| {
            matches!(
                m,
                WM_LBUTTONDOWN
                    | WM_RBUTTONDOWN
                    | WM_MBUTTONDOWN
                    | WM_NCLBUTTONDOWN
                    | WM_NCRBUTTONDOWN
                    | WM_NCMBUTTONDOWN
            )
        });
        if press {
            let _ = guard(|| {
                CURRENT.with(|c| {
                    if let Some(state) = c.borrow().upgrade() {
                        state.forget_word();
                    }
                });
                Ok(())
            });
        }
    }
    // SAFETY: forwarding the hook call unchanged.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

#[implement(
    ITfTextInputProcessorEx,
    ITfTextInputProcessor,
    ITfKeyEventSink,
    ITfThreadMgrEventSink
)]
pub struct TextService {
    state: Rc<State>,
    _dll: DllRef,
}

// Hand-written: never print engine state (it holds the word being typed).
impl std::fmt::Debug for TextService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextService")
            .field("in_word", &self.state.in_word())
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

fn key_input(vk: u16, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send(inputs: &[INPUT]) -> bool {
    if inputs.is_empty() {
        return true;
    }
    let size = i32::try_from(size_of::<INPUT>()).unwrap_or(0);
    // SAFETY: well-formed keyboard inputs.
    let sent = unsafe { SendInput(inputs, size) };
    sent as usize == inputs.len()
}

/// The keys that turn `old` on screen into `new`: one Backspace per character
/// after their common start, then the rest of `new` as Unicode characters.
/// All text is precomposed, so one Backspace removes one character.
///
/// Backspaces use scan code 0, which no physical key does: that is how
/// `classify_inner` recognises them, along with `VK_PACKET`.
fn diff_inputs(old: &str, new: &str) -> Vec<INPUT> {
    let common = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .count();
    let mut inputs = Vec::new();
    for _ in common..old.chars().count() {
        inputs.push(key_input(VK_BACK.0, 0, KEYBD_EVENT_FLAGS(0)));
        inputs.push(key_input(VK_BACK.0, 0, KEYEVENTF_KEYUP));
    }
    let rest: String = new.chars().skip(common).collect();
    for unit in rest.encode_utf16() {
        inputs.push(key_input(0, unit, KEYEVENTF_UNICODE));
        inputs.push(key_input(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
    }
    inputs
}

/// Send the key the user just pressed once more, so the app sees it after any
/// correction to the word. The word is over by then, so bikey ignores it.
fn resend_key(wparam: WPARAM, lparam: LPARAM) {
    let Ok(vk) = u16::try_from(wparam.0) else {
        return;
    };
    let scan = u16::try_from((lparam.0 >> 16) & 0xff).unwrap_or(0);
    let flags = if (lparam.0 >> 24) & 1 == 1 {
        KEYEVENTF_EXTENDEDKEY
    } else {
        KEYBD_EVENT_FLAGS(0)
    };
    send(&[key_input(vk, scan, flags)]);
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
    fn in_word(&self) -> bool {
        self.engine.borrow().is_composing()
    }

    /// Forget the word, leaving whatever is on screen untouched.
    fn forget_word(&self) {
        self.engine.borrow_mut().reset();
        self.shown.borrow_mut().clear();
        self.finished.borrow_mut().take();
        self.span.set(None);
    }

    /// Whether Esc would change anything right now.
    fn can_restore(&self) -> bool {
        if self.in_word() {
            self.engine.borrow().typed() != *self.shown.borrow()
        } else {
            self.finished.borrow().is_some()
        }
    }

    /// Swap the word back to the keys that were typed. The word is over after this.
    fn restore(&self) -> bool {
        let inputs = if self.in_word() {
            diff_inputs(&self.shown.borrow(), &self.engine.borrow().typed())
        } else if let Some(f) = self.finished.borrow().as_ref() {
            diff_inputs(
                &format!("{}{}", f.word, f.ended_by),
                &format!("{}{}", f.typed, f.ended_by),
            )
        } else {
            return false;
        };
        let done = send(&inputs);
        self.forget_word();
        done
    }

    fn enabled(&self) -> bool {
        let tm = self.thread_mgr.borrow();
        settings::enabled()
            && tm.as_ref().is_some_and(|tm| {
                tsf::compartment_i32(tm, &GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) != Some(0)
            })
    }

    /// If the caret has left the word (a mouse click, say), the word is over.
    /// Correcting it now would delete the wrong text. Apps can report a caret that
    /// lags behind typing, so only a caret outside the word's span counts.
    fn check_caret(&self, ctx: &ITfContext) {
        let Some(span) = self.span.get() else {
            return;
        };
        let moved = tsf::caret(ctx, self.client_id.get())
            .is_some_and(|(at, len)| len != 0 || at < span.start || at > span.end);
        if moved {
            self.forget_word();
        }
    }

    fn classify(&self, ctx: &ITfContext, wparam: WPARAM, lparam: LPARAM) -> Action {
        let Ok(vk) = u16::try_from(wparam.0) else {
            return Action::Ignore;
        };
        // bikey's own output comes back through here.
        let scan = (lparam.0 >> 16) & 0xff;
        if vk == VK_PACKET.0 || (vk == VK_BACK.0 && scan == 0) || is_modifier(vk) {
            return Action::Ignore;
        }
        // A shortcut ends the word as it stands. No corrections: an injected
        // Backspace would combine with the held key (Ctrl+Backspace deletes a word).
        if [VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
            .iter()
            .any(|k| key_down(k.0))
        {
            self.forget_word();
            return Action::Ignore;
        }
        self.check_caret(ctx);
        // Any other key ends the chance to restore the word just finished.
        if vk != VK_ESCAPE.0 {
            self.finished.borrow_mut().take();
        }
        let in_word = self.in_word();
        if !in_word {
            // Settings can change in any app; they apply from the next word.
            self.engine.borrow_mut().set_config(settings::config());
        }
        let action = if vk == VK_ESCAPE.0 && self.can_restore() {
            Action::Restore
        } else if vk == VK_BACK.0 {
            if !in_word {
                return Action::Ignore;
            }
            Action::Backspace
        } else {
            match key_char(u32::from(vk), lparam) {
                Some(c) if self.engine.borrow().accepts(c) => Action::Word(c),
                Some(c) if in_word && !c.is_control() => Action::CommitWith(c),
                // Under the Vietnamese layout the app would type the wrong character.
                Some(c) if vietnamese_layout() && us_key(u32::from(vk), false).is_some() => {
                    Action::CommitWith(c)
                }
                _ if in_word => Action::Commit,
                _ => return Action::Ignore,
            }
        };
        // Only now pay for the field checks, and only for keys we would take.
        if !self.enabled() || tsf::is_secure_field(ctx, self.client_id.get()) {
            self.forget_word();
            return Action::Ignore;
        }
        action
    }

    /// Turn the word on screen into `text`. On failure the word is forgotten and
    /// false returned, so the key can go to the app instead.
    fn type_word(&self, text: &str) -> bool {
        let inputs = diff_inputs(&self.shown.borrow(), text);
        if !send(&inputs) {
            self.forget_word();
            return false;
        }
        true
    }

    fn word_key(&self, ctx: &ITfContext, c: char) -> bool {
        if !self.in_word() {
            // A selection is replaced by the first character, like normal typing.
            let start = tsf::caret(ctx, self.client_id.get()).map(|(at, _)| at);
            self.span.set(start.map(|start| Span { start, end: start }));
        }
        let Some(out) = self.engine.borrow_mut().key(c) else {
            return false;
        };
        let whole = format!("{}{}", out.commit, out.preedit);
        self.update(&whole, &out.preedit)
    }

    fn backspace(&self) -> bool {
        let Some(out) = self.engine.borrow_mut().backspace() else {
            return false;
        };
        self.update(&out.preedit, &out.preedit)
    }

    /// Type `text` over the word, leaving `word` as the word in progress.
    fn update(&self, text: &str, word: &str) -> bool {
        if !self.type_word(text) {
            return false;
        }
        if let Some(mut span) = self.span.get() {
            let old = i32::try_from(self.shown.borrow().encode_utf16().count()).unwrap_or(0);
            let new = i32::try_from(text.encode_utf16().count()).unwrap_or(0);
            span.end += new - old;
            self.span.set(Some(span));
        }
        word.clone_into(&mut self.shown.borrow_mut());
        if word.is_empty() {
            self.span.set(None);
        }
        true
    }

    /// Finish the word with its final text (English fix-ups happen here), plus `c`.
    fn commit(&self, c: Option<char>) -> bool {
        let typed = self.engine.borrow().typed();
        let word = self.engine.borrow_mut().commit();
        let text: String = word.chars().chain(c).collect();
        let done = self.type_word(&text);
        let span = self.span.get();
        self.forget_word();
        if let (true, Some(ended_by)) = (done && !typed.is_empty() && word != typed, c) {
            // The caret check keeps guarding this word until the chance to restore ends.
            let length = i32::try_from(text.encode_utf16().count()).unwrap_or(0);
            self.span.set(span.map(|s| Span {
                start: s.start,
                end: s.start + length,
            }));
            *self.finished.borrow_mut() = Some(Finished {
                word,
                typed,
                ended_by,
            });
        }
        done
    }

    fn deactivate(&self) {
        self.forget_word();
        if let Some(hook) = self.mouse_hook.take() {
            // SAFETY: installed in ActivateEx on this thread.
            let _ = unsafe { UnhookWindowsHookEx(hook) };
        }
        CURRENT.with(|c| *c.borrow_mut() = Weak::new());
        let tm = self.thread_mgr.borrow_mut().take();
        let button = self.button.borrow_mut().take();
        if let Some(tm) = tm {
            let id = self.client_id.get();
            if let (Some(button), Ok(items)) = (button, tm.cast::<ITfLangBarItemMgr>()) {
                // SAFETY: removing the item added in ActivateEx.
                let _ = unsafe { items.RemoveItem(&button) };
            }
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
            state.engine.borrow_mut().set_config(settings::config());
            CURRENT.with(|c| *c.borrow_mut() = Rc::downgrade(state));
            // SAFETY: a hook for this thread only, removed in `deactivate`.
            let hook = unsafe {
                SetWindowsHookExW(WH_MOUSE, Some(mouse_proc), None, GetCurrentThreadId())
            }?;
            state.mouse_hook.set(Some(hook));

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
            // bikey's own on/off is the saved setting; this just says the keyboard is open.
            tsf::set_compartment_i32(&tm, &GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, tid, 1)?;

            let weak = Rc::downgrade(state);
            let button = ModeButton::create(Rc::new(move || {
                if let Some(state) = weak.upgrade() {
                    state.forget_word();
                }
            }));
            let items: ITfLangBarItemMgr = tm.cast()?;
            // SAFETY: plain COM call; removed in `deactivate`.
            unsafe { items.AddItem(&button)? };
            *state.button.borrow_mut() = Some(button);
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
            let ours = !matches!(self.state.classify(ctx, wparam, lparam), Action::Ignore);
            Ok(ours.into())
        })
    }

    fn OnKeyDown(&self, pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        guard(|| {
            let ctx = pic.ok()?;
            let state = &self.state;
            let eaten = match state.classify(ctx, wparam, lparam) {
                Action::Word(c) => state.word_key(ctx, c),
                Action::Backspace => state.backspace(),
                Action::CommitWith(c) => state.commit(Some(c)),
                Action::Restore => state.restore(),
                Action::Commit => {
                    state.commit(None);
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
            let button = self.state.button.borrow().clone();
            if let Some(button) = button {
                langbar::toggle(&button)?;
            }
            Ok(true.into())
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
        // Focus moved to another field: the word stays where it was typed.
        guard(|| {
            self.state.forget_word();
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
