//! Input, in two tiers.
//!
//! **Posted** messages go straight into one window's queue. The cursor stays
//! where the person left it and the foreground doesn't change. Many apps honour
//! them; XAML apps and some custom-drawn ones don't. For clicks that's caught
//! afterwards by reading the window back (`run_verified_click` in the app).
//!
//! **SendInput** is the last resort, and only with `allow_physical`: it needs
//! the target in front and the real cursor on it, because that is what real
//! input means. Both are put back afterwards in background mode. It can't reach
//! an elevated window and Windows drops the events with no error, so that is
//! asked first.

use super::window_list::{
    activate_window, cursor_point, deepest_child, focused_child, foreground_window, hwnd_from_id,
    id_from_hwnd, list_window_records, process_is_elevated, window_exists, window_thread,
};
use crate::input_tier::InputOutcome;
use crate::window_target::{physical_forced, pinned, window_at_point};
use crate::AutomationError;
use std::time::Duration;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardState, MapVirtualKeyW, SendInput, SetKeyboardState, VkKeyScanW, INPUT, INPUT_0,
    INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, MOUSE_EVENT_FLAGS, VIRTUAL_KEY, VK_BACK,
    VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1, VK_HOME, VK_INSERT,
    VK_LEFT, VK_LWIN, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_SPACE, VK_TAB,
    VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    PostMessageW, SetCursorPos, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

// The button and modifier bits a mouse message's wparam carries.
const MK_LBUTTON: usize = 0x0001;
const MK_RBUTTON: usize = 0x0002;
const MK_SHIFT: usize = 0x0004;
const MK_CONTROL: usize = 0x0008;
const MK_MBUTTON: usize = 0x0010;

/// A wheel notch, in the units Windows counts them.
const WHEEL_DELTA: i32 = 120;

/// How long a posted shortcut's key state stays set. A posted message is
/// handled whenever the target gets to it, and nothing reports back that it
/// has, so this is the one place input waits a fixed time: long enough for a
/// message loop that isn't hung, short enough not to be noticed.
const POSTED_SHORTCUT_SETTLE: Duration = Duration::from_millis(50);

/// Pixels between the steps of a real drag, so the app sees it as one.
const DRAG_STEPS: i32 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
}

impl Button {
    fn messages(self) -> (u32, u32, usize) {
        match self {
            Button::Left => (WM_LBUTTONDOWN, WM_LBUTTONUP, MK_LBUTTON),
            Button::Right => (WM_RBUTTONDOWN, WM_RBUTTONUP, MK_RBUTTON),
            Button::Middle => (WM_MBUTTONDOWN, WM_MBUTTONUP, MK_MBUTTON),
        }
    }

    fn events(self) -> (MOUSE_EVENT_FLAGS, MOUSE_EVENT_FLAGS) {
        match self {
            Button::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
            Button::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
            Button::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
        }
    }
}

// ── Keys ────────────────────────────────────────────────────────────────────

/// A key and the modifiers held with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyCombo {
    pub modifiers: Vec<VIRTUAL_KEY>,
    pub key: VIRTUAL_KEY,
}

fn modifier_key(name: &str) -> Option<VIRTUAL_KEY> {
    Some(match name {
        "ctrl" | "control" => VK_CONTROL,
        "shift" => VK_SHIFT,
        "alt" | "option" => VK_MENU,
        "win" | "windows" | "super" | "meta" | "superl" | "superr" => VK_LWIN,
        // A model that learned its shortcuts on a Mac means Ctrl by this.
        "cmd" | "command" => VK_CONTROL,
        _ => return None,
    })
}

fn named_key(name: &str) -> Option<VIRTUAL_KEY> {
    Some(match name {
        "return" | "enter" | "kpenter" => VK_RETURN,
        "tab" => VK_TAB,
        "escape" | "esc" => VK_ESCAPE,
        "space" => VK_SPACE,
        "backspace" => VK_BACK,
        "delete" | "del" => VK_DELETE,
        "insert" | "ins" => VK_INSERT,
        "home" => VK_HOME,
        "end" => VK_END,
        "pageup" | "prior" => VK_PRIOR,
        "pagedown" | "next" => VK_NEXT,
        "up" | "arrowup" => VK_UP,
        "down" | "arrowdown" => VK_DOWN,
        "left" | "arrowleft" => VK_LEFT,
        "right" | "arrowright" => VK_RIGHT,
        "capslock" => VK_CAPITAL,
        other => {
            let number: u16 = other.strip_prefix('f')?.parse().ok()?;
            if !(1..=24).contains(&number) {
                return None;
            }
            VIRTUAL_KEY(VK_F1.0 + number - 1)
        }
    })
}

fn character_key(name: &str) -> Option<(VIRTUAL_KEY, bool)> {
    let mut chars = name.chars();
    let character = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    let mut unit = [0u16; 2];
    if character.encode_utf16(&mut unit).len() != 1 {
        return None;
    }
    // SAFETY: takes a character by value and reads the keyboard layout.
    let scan = unsafe { VkKeyScanW(unit[0]) };
    if scan == -1 {
        return None;
    }
    let scan = scan as u16;
    Some((VIRTUAL_KEY(scan & 0xff), scan & 0x0100 != 0))
}

/// Parse `ctrl+s`, `Return`, `shift+Tab`. Spelling is forgiving (`Page_Up`,
/// `page-up`), since it comes from a model.
pub fn parse_key(text: &str) -> Result<KeyCombo, AutomationError> {
    let parts: Vec<&str> = if text == "+" {
        vec!["+"]
    } else {
        text.split('+').filter(|p| !p.is_empty()).collect()
    };
    let Some((last, rest)) = parts.split_last() else {
        return Err(AutomationError::InvalidArgument(
            "No key was named".to_string(),
        ));
    };
    let normalize = |s: &str| s.to_lowercase().replace(['_', '-'], "");
    let mut modifiers = Vec::new();
    for part in rest {
        modifiers.push(modifier_key(&normalize(part)).ok_or_else(|| {
            AutomationError::InvalidArgument(format!("'{part}' isn't a modifier key"))
        })?);
    }
    let name = normalize(last);
    let key = if let Some(key) = named_key(&name).or_else(|| modifier_key(&name)) {
        key
    } else if let Some((key, needs_shift)) = character_key(last) {
        if needs_shift && !modifiers.contains(&VK_SHIFT) {
            modifiers.push(VK_SHIFT);
        }
        key
    } else {
        return Err(AutomationError::InvalidArgument(format!(
            "'{last}' isn't a key this keyboard has"
        )));
    };
    Ok(KeyCombo { modifiers, key })
}

fn is_extended(key: VIRTUAL_KEY) -> bool {
    matches!(
        key,
        VK_DELETE
            | VK_INSERT
            | VK_HOME
            | VK_END
            | VK_PRIOR
            | VK_NEXT
            | VK_UP
            | VK_DOWN
            | VK_LEFT
            | VK_RIGHT
            | VK_LWIN
    )
}

fn scan_code(key: VIRTUAL_KEY) -> u32 {
    // SAFETY: takes values only and reads the keyboard layout.
    unsafe { MapVirtualKeyW(u32::from(key.0), MAPVK_VK_TO_VSC) }
}

// ── Where input goes ────────────────────────────────────────────────────────

/// The error every path gives for a window Windows won't let Juno touch.
fn refuse_if_elevated(hwnd: HWND) -> Result<(), AutomationError> {
    if process_is_elevated(super::window_list::app_pid(hwnd)) {
        return Err(AutomationError::ElevatedWindow);
    }
    Ok(())
}

/// The window a pointer action is for: the pinned one, else the topmost that
/// isn't Juno. Remembered, so keystrokes that follow go to the same place.
fn pointer_target(x: f64, y: f64) -> Result<Option<HWND>, AutomationError> {
    let (hwnd, pid) = match pinned() {
        Some(pin) => (hwnd_from_id(pin.window_id), pin.pid),
        None => {
            let records = list_window_records();
            let own = std::process::id() as i32;
            match window_at_point(&records, x, y, own) {
                Some(window) => (hwnd_from_id(window.id), window.pid),
                None => return Ok(None),
            }
        }
    };
    refuse_if_elevated(hwnd)?;
    crate::background::remember_target_window(pid, Some(id_from_hwnd(hwnd)));
    Ok(Some(hwnd))
}

/// The window keystrokes are for, when one has been targeted.
fn keyboard_target() -> Result<Option<HWND>, AutomationError> {
    let Some(target) = crate::background::input_target() else {
        return Ok(None);
    };
    let hwnd = match target.window_id {
        Some(id) => hwnd_from_id(id),
        None => match super::window_list::top_window_of_pid(target.pid) {
            Some(hwnd) => hwnd,
            None => return Ok(None),
        },
    };
    if !window_exists(hwnd) {
        return Ok(None);
    }
    refuse_if_elevated(hwnd)?;
    Ok(Some(hwnd))
}

/// The inner window keystrokes should be posted to. A field the agent pressed
/// or named wins over the window's own idea of its focus.
fn typing_window(root: HWND) -> HWND {
    super::ax::remembered_field_window(root).unwrap_or_else(|| focused_child(root))
}

// ── Posted input ────────────────────────────────────────────────────────────

fn point_lparam(x: i32, y: i32) -> LPARAM {
    LPARAM(((y & 0xffff) << 16 | (x & 0xffff)) as isize)
}

fn client_point(hwnd: HWND, x: i32, y: i32) -> (i32, i32) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::ScreenToClient;
    let mut point = POINT { x, y };
    // SAFETY: `point` is a valid, writable POINT.
    let _ = unsafe { ScreenToClient(hwnd, &mut point) };
    (point.x, point.y)
}

fn post(hwnd: HWND, message: u32, wparam: usize, lparam: LPARAM) -> Result<(), AutomationError> {
    // SAFETY: posts plain values with no pointers in them; a stale handle only
    // makes the call fail.
    unsafe { PostMessageW(Some(hwnd), message, WPARAM(wparam), lparam) }
        .map_err(|e| AutomationError::PlatformError(format!("The window refused the message: {e}")))
}

fn modifier_state(modifiers: Option<&str>) -> usize {
    match modifiers.map(str::to_lowercase).as_deref() {
        Some("shift") => MK_SHIFT,
        Some("ctrl" | "control" | "cmd" | "command") => MK_CONTROL,
        _ => 0,
    }
}

fn post_click(
    root: HWND,
    button: Button,
    x: f64,
    y: f64,
    count: u32,
    modifiers: Option<&str>,
) -> Result<(), AutomationError> {
    let (sx, sy) = (x.round() as i32, y.round() as i32);
    let target = deepest_child(root, sx, sy);
    let (cx, cy) = client_point(target, sx, sy);
    let lparam = point_lparam(cx, cy);
    let held = modifier_state(modifiers);
    let (down, up, pressed) = button.messages();
    post(target, WM_MOUSEMOVE, held, lparam)?;
    for press in 0..count.max(1) {
        // Windows turns the second press of a real double click into a
        // double-click message, and only for classes that asked for one. Apps
        // that didn't ask ignore this and count presses.
        let message = if press == 1 && button == Button::Left {
            WM_LBUTTONDBLCLK
        } else {
            down
        };
        post(target, message, held | pressed, lparam)?;
        post(target, up, held, lparam)?;
    }
    Ok(())
}

fn post_mouse_button(
    root: HWND,
    button: Button,
    x: f64,
    y: f64,
    down: bool,
) -> Result<(), AutomationError> {
    let (sx, sy) = (x.round() as i32, y.round() as i32);
    let target = deepest_child(root, sx, sy);
    let (cx, cy) = client_point(target, sx, sy);
    let (down_message, up_message, pressed) = button.messages();
    if down {
        post(target, down_message, pressed, point_lparam(cx, cy))
    } else {
        post(target, up_message, 0, point_lparam(cx, cy))
    }
}

fn post_drag(root: HWND, from: (f64, f64), to: (f64, f64)) -> Result<(), AutomationError> {
    let (sx, sy) = (from.0.round() as i32, from.1.round() as i32);
    let target = deepest_child(root, sx, sy);
    let (down, up, pressed) = Button::Left.messages();
    let (cx, cy) = client_point(target, sx, sy);
    post(target, WM_MOUSEMOVE, 0, point_lparam(cx, cy))?;
    post(target, down, pressed, point_lparam(cx, cy))?;
    for step in 1..=DRAG_STEPS {
        let t = f64::from(step) / f64::from(DRAG_STEPS);
        let (px, py) = client_point(
            target,
            (from.0 + (to.0 - from.0) * t).round() as i32,
            (from.1 + (to.1 - from.1) * t).round() as i32,
        );
        post(target, WM_MOUSEMOVE, pressed, point_lparam(px, py))?;
    }
    let (px, py) = client_point(target, to.0.round() as i32, to.1.round() as i32);
    post(target, up, 0, point_lparam(px, py))
}

fn wheel(direction: &str, amount: f64) -> Result<(u32, i32), AutomationError> {
    let notches = (amount.abs().round() as i32)
        .max(1)
        .saturating_mul(WHEEL_DELTA);
    match direction.to_lowercase().as_str() {
        "up" => Ok((WM_MOUSEWHEEL, notches)),
        "down" => Ok((WM_MOUSEWHEEL, -notches)),
        "right" => Ok((WM_MOUSEHWHEEL, notches)),
        "left" => Ok((WM_MOUSEHWHEEL, -notches)),
        other => Err(AutomationError::InvalidArgument(format!(
            "'{other}' isn't a scroll direction"
        ))),
    }
}

fn post_scroll(
    root: HWND,
    x: f64,
    y: f64,
    direction: &str,
    amount: f64,
) -> Result<(), AutomationError> {
    let (message, delta) = wheel(direction, amount)?;
    let (sx, sy) = (x.round() as i32, y.round() as i32);
    let target = deepest_child(root, sx, sy);
    // The wheel messages are the one kind that carry screen coordinates.
    let wparam = (delta as i16 as u16 as usize) << 16;
    post(target, message, wparam, point_lparam(sx, sy))
}

fn key_lparam(key: VIRTUAL_KEY, up: bool, alt: bool) -> LPARAM {
    let mut bits: u32 = 1 | (scan_code(key) << 16);
    if is_extended(key) {
        bits |= 1 << 24;
    }
    if alt {
        bits |= 1 << 29;
    }
    if up {
        bits |= (1 << 30) | (1 << 31);
    }
    LPARAM(bits as isize)
}

fn post_key_event(
    target: HWND,
    key: VIRTUAL_KEY,
    up: bool,
    alt: bool,
) -> Result<(), AutomationError> {
    let message = match (up, alt) {
        (false, false) => WM_KEYDOWN,
        (true, false) => WM_KEYUP,
        (false, true) => WM_SYSKEYDOWN,
        (true, true) => WM_SYSKEYUP,
    };
    post(
        target,
        message,
        usize::from(key.0),
        key_lparam(key, up, alt),
    )
}

/// Post a key with modifiers "held" for the target's thread alone. Accelerators
/// read the thread's key state when they see the message, and that state can be
/// set from outside by joining the thread's input queue. Nothing else on the
/// machine sees these modifiers, and the person's own keyboard isn't touched.
fn post_key_with_modifiers(
    target: HWND,
    combo: &KeyCombo,
    hold_down_only: bool,
) -> Result<(), AutomationError> {
    let alt = combo.modifiers.contains(&VK_MENU);
    let thread = window_thread(target);
    // SAFETY: the attachment is undone below on every path past it, and the key
    // state buffers are the 256 bytes the calls require.
    let ours = unsafe { GetCurrentThreadId() };
    let attached =
        thread != 0 && thread != ours && unsafe { AttachThreadInput(ours, thread, true) }.as_bool();
    let mut saved = [0u8; 256];
    let read = unsafe { GetKeyboardState(&mut saved) }.is_ok();
    if read {
        let mut state = saved;
        for modifier in &combo.modifiers {
            state[usize::from(modifier.0)] |= 0x80;
        }
        let _ = unsafe { SetKeyboardState(&state) };
    }
    let mut result = post_key_event(target, combo.key, false, alt);
    if !hold_down_only && result.is_ok() {
        result = post_key_event(target, combo.key, true, alt);
    }
    std::thread::sleep(POSTED_SHORTCUT_SETTLE);
    if read {
        let _ = unsafe { SetKeyboardState(&saved) };
    }
    if attached {
        let _ = unsafe { AttachThreadInput(ours, thread, false) };
    }
    result
}

fn post_key_combo(root: HWND, combo: &KeyCombo) -> Result<(), AutomationError> {
    let target = typing_window(root);
    if combo.modifiers.is_empty() {
        post_key_event(target, combo.key, false, false)?;
        post_key_event(target, combo.key, true, false)
    } else {
        post_key_with_modifiers(target, combo, false)
    }
}

/// Type by posting characters to the field. Nothing moves. The app can still
/// ignore them, and this can't tell: the accessibility path is tried first
/// because it can.
fn post_text(root: HWND, text: &str) -> Result<(), AutomationError> {
    let target = typing_window(root);
    for unit in text.encode_utf16() {
        let unit = if unit == u16::from(b'\n') {
            u16::from(b'\r')
        } else {
            unit
        };
        post(target, WM_CHAR, usize::from(unit), LPARAM(1))?;
    }
    Ok(())
}

// ── Real input ──────────────────────────────────────────────────────────────

fn send(inputs: &[INPUT]) -> Result<(), AutomationError> {
    // SAFETY: `inputs` is a valid slice and the size is that of its elements.
    let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        // UIPI blocks injection into a higher-integrity foreground this way.
        Err(AutomationError::PlatformError(
            "Windows blocked the input. Something on screen may be running as administrator"
                .to_string(),
        ))
    }
}

fn mouse_input(flags: MOUSE_EVENT_FLAGS, data: i32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: data as u32,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn key_input(key: VIRTUAL_KEY, up: bool) -> INPUT {
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    if is_extended(key) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: scan_code(key) as u16,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn unicode_input(unit: u16, up: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn move_cursor(x: f64, y: f64) -> Result<(), AutomationError> {
    // SAFETY: takes values only.
    unsafe { SetCursorPos(x.round() as i32, y.round() as i32) }
        .map_err(|e| AutomationError::PlatformError(format!("Couldn't move the cursor: {e}")))
}

fn modifier_keys(modifiers: Option<&str>) -> Vec<VIRTUAL_KEY> {
    modifiers
        .map(|text| {
            text.split('+')
                .filter_map(|m| modifier_key(&m.trim().to_lowercase()))
                .collect()
        })
        .unwrap_or_default()
}

fn with_modifiers_held(
    modifiers: &[VIRTUAL_KEY],
    act: impl FnOnce() -> Result<(), AutomationError>,
) -> Result<(), AutomationError> {
    let downs: Vec<INPUT> = modifiers.iter().map(|m| key_input(*m, false)).collect();
    if !downs.is_empty() {
        send(&downs)?;
    }
    let result = act();
    let ups: Vec<INPUT> = modifiers
        .iter()
        .rev()
        .map(|m| key_input(*m, true))
        .collect();
    if !ups.is_empty() {
        // Always tried, so a failed click can't leave Ctrl stuck down.
        let released = send(&ups);
        return result.and(released);
    }
    result
}

/// What real input took from the person, and hands back when it's done.
struct Borrowed {
    cursor: Option<(i32, i32)>,
    foreground: Option<HWND>,
}

/// Put the target in front for real input, remembering what to give back. With
/// background mode off nothing is given back: the person asked for the agent to
/// use the real cursor.
fn borrow_for(target: Option<HWND>) -> Result<Borrowed, AutomationError> {
    let background = crate::background::is_background_mode();
    let borrowed = Borrowed {
        cursor: background.then(cursor_point).flatten(),
        foreground: background.then(foreground_window).flatten(),
    };
    if let Some(hwnd) = target {
        if !activate_window(hwnd) {
            tracing::warn!(
                "Couldn't bring window {:?} to the front for real input",
                hwnd
            );
        }
    }
    // Real input goes to whatever is in front, whatever we asked for above.
    if let Some(front) = foreground_window() {
        refuse_if_elevated(front)?;
    }
    Ok(borrowed)
}

fn give_back(borrowed: Borrowed, keep_foreground: bool) {
    if let Some((x, y)) = borrowed.cursor {
        // SAFETY: takes values only.
        let _ = unsafe { SetCursorPos(x, y) };
    }
    if keep_foreground {
        return;
    }
    if let Some(previous) = borrowed.foreground {
        if foreground_window() != Some(previous) && window_exists(previous) {
            activate_window(previous);
        }
    }
}

/// Run real input and give back the cursor and foreground, whatever it did.
fn with_real_input(
    target: Option<HWND>,
    keep_foreground: bool,
    act: impl FnOnce() -> Result<(), AutomationError>,
) -> Result<(), AutomationError> {
    let borrowed = borrow_for(target)?;
    let result = act();
    give_back(borrowed, keep_foreground);
    result
}

/// A click where the cursor is pointed, with modifiers, and nothing else.
pub fn send_click(
    button: Button,
    x: f64,
    y: f64,
    count: u32,
    modifiers: Option<&str>,
) -> Result<(), AutomationError> {
    let (down, up) = button.events();
    let keys = modifier_keys(modifiers);
    move_cursor(x, y)?;
    with_modifiers_held(&keys, || {
        for _ in 0..count.max(1) {
            send(&[mouse_input(down, 0), mouse_input(up, 0)])?;
        }
        Ok(())
    })
}

pub fn send_mouse_button(
    button: Button,
    x: f64,
    y: f64,
    press: bool,
) -> Result<(), AutomationError> {
    let (down, up) = button.events();
    move_cursor(x, y)?;
    send(&[mouse_input(if press { down } else { up }, 0)])
}

pub fn send_drag(from: (f64, f64), to: (f64, f64)) -> Result<(), AutomationError> {
    let (down, up) = Button::Left.events();
    move_cursor(from.0, from.1)?;
    send(&[mouse_input(down, 0)])?;
    for step in 1..=DRAG_STEPS {
        let t = f64::from(step) / f64::from(DRAG_STEPS);
        move_cursor(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)?;
        std::thread::sleep(Duration::from_millis(8));
    }
    send(&[mouse_input(up, 0)])
}

pub fn send_scroll(direction: &str, amount: f64) -> Result<(), AutomationError> {
    let (message, delta) = wheel(direction, amount)?;
    let flags = if message == WM_MOUSEHWHEEL {
        MOUSEEVENTF_HWHEEL
    } else {
        MOUSEEVENTF_WHEEL
    };
    send(&[mouse_input(flags, delta)])
}

pub fn send_key_combo(combo: &KeyCombo) -> Result<(), AutomationError> {
    with_modifiers_held(&combo.modifiers, || {
        send(&[key_input(combo.key, false), key_input(combo.key, true)])
    })
}

pub fn send_key_edge(combo: &KeyCombo, up: bool) -> Result<(), AutomationError> {
    let mut inputs: Vec<INPUT> = Vec::new();
    if up {
        inputs.push(key_input(combo.key, true));
        inputs.extend(combo.modifiers.iter().rev().map(|m| key_input(*m, true)));
    } else {
        inputs.extend(combo.modifiers.iter().map(|m| key_input(*m, false)));
        inputs.push(key_input(combo.key, false));
    }
    send(&inputs)
}

pub fn send_text(text: &str) -> Result<(), AutomationError> {
    let inputs: Vec<INPUT> = text
        .encode_utf16()
        .flat_map(|unit| {
            let unit = if unit == u16::from(b'\n') {
                u16::from(b'\r')
            } else {
                unit
            };
            [unicode_input(unit, false), unicode_input(unit, true)]
        })
        .collect();
    if inputs.is_empty() {
        return Ok(());
    }
    send(&inputs)
}

// ── The engine's no-warp entry points ───────────────────────────────────────

/// Whether this step should be real input from the start: background mode off,
/// or the retry after an app ignored a posted click.
fn real_input_wanted() -> bool {
    !crate::background::is_background_mode() || physical_forced()
}

/// A pointer step, posted when it can be and real when it must be. `Ok(None)`
/// is the caller's cue to ask for the mouse.
fn pointer_step(
    x: f64,
    y: f64,
    allow_physical: bool,
    keep_foreground: bool,
    method: &'static str,
    posted: impl FnOnce(HWND) -> Result<(), AutomationError>,
    real: impl FnOnce() -> Result<(), AutomationError>,
) -> Result<Option<InputOutcome>, AutomationError> {
    let target = pointer_target(x, y)?;
    if !real_input_wanted() {
        if let Some(hwnd) = target {
            posted(hwnd)?;
            return Ok(Some(InputOutcome::process_targeted(method)));
        }
    }
    if !allow_physical {
        return Ok(None);
    }
    with_real_input(target, keep_foreground, real)?;
    Ok(Some(InputOutcome::physical_cursor("SendInput")))
}

pub fn click_no_warp(
    button: Button,
    x: f64,
    y: f64,
    count: u32,
    modifiers: Option<&str>,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    pointer_step(
        x,
        y,
        allow_physical,
        // A menu a real right click opened closes the moment its window loses
        // the foreground, which would undo the click.
        button == Button::Right,
        "PostMessage",
        |hwnd| post_click(hwnd, button, x, y, count, modifiers),
        || send_click(button, x, y, count, modifiers),
    )
}

pub fn mouse_button_no_warp(
    x: f64,
    y: f64,
    press: bool,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    pointer_step(
        x,
        y,
        allow_physical,
        // A press with no release yet is half a gesture, so it stays in front
        // until the release.
        press,
        "PostMessage",
        |hwnd| post_mouse_button(hwnd, Button::Left, x, y, press),
        || send_mouse_button(Button::Left, x, y, press),
    )
}

pub fn drag_no_warp(
    from: (f64, f64),
    to: (f64, f64),
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    pointer_step(
        from.0,
        from.1,
        allow_physical,
        false,
        "PostMessage",
        |hwnd| post_drag(hwnd, from, to),
        || send_drag(from, to),
    )
}

pub fn scroll_no_warp(
    x: f64,
    y: f64,
    direction: &str,
    amount: f64,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    let target = pointer_target(x, y)?;
    if !real_input_wanted() {
        if let Some(hwnd) = target {
            if super::ax::scroll_at(hwnd, x, y, direction, amount).is_ok() {
                return Ok(Some(InputOutcome::accessibility("UIA Scroll")));
            }
            post_scroll(hwnd, x, y, direction, amount)?;
            return Ok(Some(InputOutcome::process_targeted("WM_MOUSEWHEEL")));
        }
    }
    if !allow_physical {
        return Ok(None);
    }
    with_real_input(target, false, || {
        move_cursor(x, y)?;
        send_scroll(direction, amount)
    })?;
    Ok(Some(InputOutcome::physical_cursor("SendInput")))
}

/// A shortcut Windows itself owns can't be posted to a window: it means nothing
/// to the app, and only real input reaches the shell.
fn needs_the_shell(combo: &KeyCombo) -> bool {
    combo.modifiers.contains(&VK_LWIN) || combo.key == VK_LWIN
}

pub fn key_no_warp(
    combo: &KeyCombo,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    let target = keyboard_target()?;
    if !real_input_wanted() && !needs_the_shell(combo) {
        if let Some(root) = target {
            post_key_combo(root, combo)?;
            return Ok(Some(InputOutcome::process_targeted("PostMessage")));
        }
    }
    if !allow_physical {
        return Ok(None);
    }
    with_real_input(target, needs_the_shell(combo), || send_key_combo(combo))?;
    Ok(Some(InputOutcome::physical_cursor("SendInput")))
}

pub fn hold_key_no_warp(
    combo: &KeyCombo,
    duration_ms: Option<u64>,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    let hold = Duration::from_millis(duration_ms.unwrap_or(0));
    let target = keyboard_target()?;
    if !real_input_wanted() && !needs_the_shell(combo) {
        if let Some(root) = target {
            let field = typing_window(root);
            post_key_with_modifiers(field, combo, true)?;
            std::thread::sleep(hold);
            post_key_event(field, combo.key, true, combo.modifiers.contains(&VK_MENU))?;
            return Ok(Some(InputOutcome::process_targeted("PostMessage")));
        }
    }
    if !allow_physical {
        return Ok(None);
    }
    with_real_input(target, false, || {
        send_key_edge(combo, false)?;
        std::thread::sleep(hold);
        send_key_edge(combo, true)
    })?;
    Ok(Some(InputOutcome::physical_cursor("SendInput")))
}

pub fn release_key_no_warp(
    combo: &KeyCombo,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    let target = keyboard_target()?;
    if !real_input_wanted() {
        if let Some(root) = target {
            let alt = combo.modifiers.contains(&VK_MENU);
            post_key_event(typing_window(root), combo.key, true, alt)?;
            return Ok(Some(InputOutcome::process_targeted("PostMessage")));
        }
    }
    if !allow_physical {
        return Ok(None);
    }
    with_real_input(target, false, || send_key_edge(combo, true))?;
    Ok(Some(InputOutcome::physical_cursor("SendInput")))
}

pub fn type_no_warp(
    text: &str,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    let target = keyboard_target()?;
    if !real_input_wanted() {
        if let Some(root) = target {
            post_text(root, text)?;
            return Ok(Some(InputOutcome::process_targeted("WM_CHAR")));
        }
    }
    if !allow_physical {
        return Ok(None);
    }
    with_real_input(target, false, || send_text(text))?;
    Ok(Some(InputOutcome::physical_cursor("SendInput")))
}
