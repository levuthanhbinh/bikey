//! Small safe wrappers around the raw TSF calls the service needs.

use core::mem::ManuallyDrop;
use std::cell::RefCell;

use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Variant::{
    VARIANT, VARIANT_0, VARIANT_0_0, VT_I4, VT_UNKNOWN, VariantClear,
};
use windows::Win32::UI::TextServices::{
    GUID_COMPARTMENT_EMPTYCONTEXT, GUID_COMPARTMENT_KEYBOARD_DISABLED, GUID_PROP_INPUTSCOPE,
    IS_ALPHANUMERIC_PIN, IS_ALPHANUMERIC_PIN_SET, IS_NUMERIC_PASSWORD, IS_NUMERIC_PIN, IS_PASSWORD,
    ITfCompartmentMgr, ITfContext, ITfEditSession, ITfEditSession_Impl, ITfInputScope, ITfRange,
    InputScope, TF_AE_NONE, TF_CONTEXT_EDIT_CONTEXT_FLAGS, TF_DEFAULT_SELECTION, TF_ES_READ,
    TF_ES_SYNC, TF_SELECTION, TF_SELECTIONSTYLE,
};
use windows_core::{GUID, Interface, Result, implement};

use crate::guard;

/// UTF-16 for TSF text APIs (length-delimited, no terminator).
pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

type SessionFn = Box<dyn FnOnce(u32) -> Result<()>>;

#[implement(ITfEditSession)]
struct Session(RefCell<Option<SessionFn>>);

impl ITfEditSession_Impl for Session_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        let f = self.0.borrow_mut().take();
        match f {
            Some(f) => guard(|| f(ec)),
            None => Ok(()),
        }
    }
}

/// Run `f` synchronously inside an edit session on `ctx`.
pub fn edit(
    ctx: &ITfContext,
    client_id: u32,
    flags: TF_CONTEXT_EDIT_CONTEXT_FLAGS,
    f: impl FnOnce(u32) -> Result<()> + 'static,
) -> Result<()> {
    let session: ITfEditSession = Session(RefCell::new(Some(Box::new(f)))).into();
    // SAFETY: plain COM call with live interfaces.
    let inner = unsafe { ctx.RequestEditSession(client_id, &session, TF_ES_SYNC | flags) }?;
    inner.ok()
}

/// Read a `VT_I4` compartment value; missing or other types read as `None`.
pub fn compartment_i32(owner: &impl Interface, guid: &GUID) -> Option<i32> {
    let mgr: ITfCompartmentMgr = owner.cast().ok()?;
    // SAFETY: plain COM calls with live interfaces.
    let mut value = unsafe { mgr.GetCompartment(guid).ok()?.GetValue().ok()? };
    // SAFETY: reading the tag, then the union member that tag selects.
    let result = unsafe {
        let v = &value.Anonymous.Anonymous;
        (v.vt == VT_I4).then_some(v.Anonymous.lVal)
    };
    // SAFETY: `value` is an initialised VARIANT we own.
    let _ = unsafe { VariantClear(&raw mut value) };
    result
}

/// A `VT_I4` VARIANT (plain integer, nothing to free).
pub fn variant_i32(value: i32) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_I4,
                Anonymous: windows::Win32::System::Variant::VARIANT_0_0_0 { lVal: value },
                ..Default::default()
            }),
        },
    }
}

pub fn set_compartment_i32(
    owner: &impl Interface,
    guid: &GUID,
    client_id: u32,
    value: i32,
) -> Result<()> {
    let mgr: ITfCompartmentMgr = owner.cast()?;
    let variant = variant_i32(value);
    // SAFETY: plain COM calls; `variant` holds a plain integer (nothing to free).
    unsafe {
        mgr.GetCompartment(guid)?
            .SetValue(client_id, &raw const variant)
    }
}

/// Whether bikey must stay out of this field: keyboard input disabled by the
/// app, an empty (non-text) context, or a password/PIN input scope.
pub fn is_secure_field(ctx: &ITfContext, client_id: u32) -> bool {
    let flag = |guid| compartment_i32(ctx, guid).is_some_and(|v| v != 0);
    if flag(&GUID_COMPARTMENT_KEYBOARD_DISABLED) || flag(&GUID_COMPARTMENT_EMPTYCONTEXT) {
        return true;
    }
    let result = std::rc::Rc::new(std::cell::Cell::new(false));
    let out = result.clone();
    let ctx2 = ctx.clone();
    let read = edit(ctx, client_id, TF_ES_READ, move |ec| {
        out.set(has_password_scope(&ctx2, ec).unwrap_or(false));
        Ok(())
    });
    // If we cannot even read the scope, err on the side of caution.
    read.is_err() || result.get()
}

fn has_password_scope(ctx: &ITfContext, ec: u32) -> Result<bool> {
    let Some(range) = selection(ctx, ec)? else {
        return Ok(false);
    };
    // SAFETY: plain COM calls with live interfaces and a valid edit cookie.
    let mut value = unsafe {
        ctx.GetProperty(&GUID_PROP_INPUTSCOPE)?
            .GetValue(ec, &range)?
    };
    // SAFETY: reading the tag, then the union member it selects; the
    // interface is cloned (AddRef) before the VARIANT is cleared.
    let unknown = unsafe {
        let v = &value.Anonymous.Anonymous;
        if v.vt == VT_UNKNOWN {
            (*v.Anonymous.punkVal).clone()
        } else {
            None
        }
    };
    // SAFETY: `value` is an initialised VARIANT we own.
    let _ = unsafe { VariantClear(&raw mut value) };
    let Some(scope) = unknown.and_then(|u| u.cast::<ITfInputScope>().ok()) else {
        return Ok(false);
    };
    let mut scopes: *mut InputScope = core::ptr::null_mut();
    let mut count = 0u32;
    // SAFETY: TSF allocates `scopes` with CoTaskMemAlloc; freed below.
    unsafe { scope.GetInputScopes(&raw mut scopes, &raw mut count)? };
    let secure = if scopes.is_null() {
        false
    } else {
        // SAFETY: TSF returned `count` initialised entries at `scopes`.
        let list = unsafe { std::slice::from_raw_parts(scopes, count as usize) };
        let found = list.iter().any(|s| {
            [
                IS_PASSWORD,
                IS_NUMERIC_PASSWORD,
                IS_NUMERIC_PIN,
                IS_ALPHANUMERIC_PIN,
                IS_ALPHANUMERIC_PIN_SET,
            ]
            .contains(s)
        });
        // SAFETY: allocated by TSF with CoTaskMemAlloc, not used after this.
        unsafe { CoTaskMemFree(Some(scopes.cast())) };
        found
    };
    Ok(secure)
}

/// The current selection (insertion point) as a range.
pub fn selection(ctx: &ITfContext, ec: u32) -> Result<Option<ITfRange>> {
    let mut sel = [TF_SELECTION::default()];
    let mut fetched = 0u32;
    // SAFETY: valid edit cookie and output buffers.
    unsafe { ctx.GetSelection(ec, TF_DEFAULT_SELECTION, &mut sel, &raw mut fetched)? };
    if fetched == 0 {
        return Ok(None);
    }
    let [TF_SELECTION { range, .. }] = sel;
    Ok(ManuallyDrop::into_inner(range))
}

/// Put the caret at `range` (collapsed by the caller as needed).
pub fn set_selection(ctx: &ITfContext, ec: u32, range: &ITfRange) -> Result<()> {
    let sel = TF_SELECTION {
        range: ManuallyDrop::new(Some(range.clone())),
        style: TF_SELECTIONSTYLE {
            ase: TF_AE_NONE,
            fInterimChar: false.into(),
        },
    };
    // SAFETY: valid edit cookie; `sel` is a well-formed selection.
    let result = unsafe { ctx.SetSelection(ec, core::slice::from_ref(&sel)) };
    drop(ManuallyDrop::into_inner(sel.range));
    result
}
