//! Top-level windows, displays and processes, read from Win32.
//!
//! A window's id is its HWND, cut to 32 bits. Windows guarantees the upper half
//! of a handle is not significant, so it survives the trip through `u32`.
//!
//! Every coordinate here is in physical pixels of the virtual desktop. Juno is
//! per-monitor DPI aware (tao turns that on; `WindowsEngine::new` turns it on
//! for the headless CLI), so UI Automation, `SendInput` and these calls all
//! agree.

use crate::errors::AutomationError;
use crate::window_target::WindowRecord;
use std::collections::HashMap;
use std::ffi::c_void;
use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, E_ACCESSDENIED, HANDLE, HWND, LPARAM, POINT, RECT, TRUE, WPARAM,
};
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, OpenProcess, OpenProcessToken,
    QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, ChildWindowFromPointEx, EnumChildWindows, EnumWindows, GetAncestor,
    GetClassNameW, GetCursorPos, GetForegroundWindow, GetGUIThreadInfo, GetWindowLongPtrW,
    GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
    PostMessageW, SetForegroundWindow, SetWindowPos, ShowWindow, CWP_SKIPINVISIBLE,
    CWP_SKIPTRANSPARENT, GA_ROOT, GUITHREADINFO, GWL_EXSTYLE, MONITORINFOF_PRIMARY,
    SET_WINDOW_POS_FLAGS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_RESTORE,
    WM_CLOSE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

/// The layer of ordinary app windows.
const LAYER_NORMAL: i32 = 0;
/// An always-on-top app window: still a window someone can mean.
const LAYER_TOPMOST: i32 = 3;
/// Shell surfaces and tool windows, which are never an input target. Outside
/// `WindowRecord::is_user_layer`.
const LAYER_SYSTEM: i32 = 25;

/// Window classes that are the desktop itself rather than an app.
const SHELL_CLASSES: &[&str] = &[
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
];

pub fn hwnd_from_id(id: u32) -> HWND {
    HWND(id as usize as *mut c_void)
}

pub fn id_from_hwnd(hwnd: HWND) -> u32 {
    hwnd.0 as usize as u32
}

pub fn class_name(hwnd: HWND) -> String {
    let mut buffer = [0u16; 256];
    // SAFETY: `buffer` is a valid, writable slice whose length goes with it. A
    // stale handle only makes the call return 0.
    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
    let length = usize::try_from(length).unwrap_or(0).min(buffer.len());
    String::from_utf16_lossy(&buffer[..length])
}

fn window_title(hwnd: HWND) -> Option<String> {
    let mut buffer = [0u16; 512];
    // SAFETY: as in `class_name`.
    let length = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    let length = usize::try_from(length).unwrap_or(0).min(buffer.len());
    let title = String::from_utf16_lossy(&buffer[..length]);
    (!title.is_empty()).then_some(title)
}

fn window_pid(hwnd: HWND) -> i32 {
    let mut pid = 0u32;
    // SAFETY: `pid` is a valid, writable u32 for the duration of the call.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    pid as i32
}

/// The thread that owns a window, which is what focus and key state hang off.
pub fn window_thread(hwnd: HWND) -> u32 {
    // SAFETY: no out-pointer is passed. A stale handle returns thread 0.
    unsafe { GetWindowThreadProcessId(hwnd, None) }
}

/// The process that is really behind a window. A UWP app such as Calculator
/// draws inside `ApplicationFrameHost`, so the frame window's own process says
/// nothing about the app: the app is the owner of its `CoreWindow` child. A
/// suspended app has no such child, and then the host is all there is.
pub fn app_pid(hwnd: HWND) -> i32 {
    if class_name(hwnd) == "ApplicationFrameWindow" {
        let mut child = None;
        unsafe extern "system" fn find_core_window(child: HWND, found: LPARAM) -> BOOL {
            if class_name(child) == "Windows.UI.Core.CoreWindow" {
                // SAFETY: `found` is the address of the `Option<HWND>` below,
                // which outlives this synchronous enumeration.
                unsafe { *(found.0 as *mut Option<HWND>) = Some(child) };
                return BOOL(0);
            }
            TRUE
        }
        // SAFETY: the callback only writes through the pointer we hand it, and
        // the enumeration finishes before `child` goes out of scope. The call
        // reports an error when the callback stops it early, which is fine.
        let _ = unsafe {
            EnumChildWindows(
                Some(hwnd),
                Some(find_core_window),
                LPARAM(&mut child as *mut Option<HWND> as isize),
            )
        };
        if let Some(core) = child {
            return window_pid(core);
        }
    }
    window_pid(hwnd)
}

/// The child window of `class` inside a window, when there is one.
pub fn child_of_class(root: HWND, prefix: &str) -> Option<HWND> {
    struct Search<'a> {
        prefix: &'a str,
        found: Option<HWND>,
    }
    unsafe extern "system" fn visit(child: HWND, search: LPARAM) -> BOOL {
        // SAFETY: `search` is the address of the `Search` below, which outlives
        // this synchronous enumeration.
        let search = unsafe { &mut *(search.0 as *mut Search) };
        if class_name(child).starts_with(search.prefix) {
            search.found = Some(child);
            return BOOL(0);
        }
        TRUE
    }
    let mut search = Search {
        prefix,
        found: None,
    };
    // SAFETY: as in `app_pid`.
    let _ = unsafe {
        EnumChildWindows(
            Some(root),
            Some(visit),
            LPARAM(&mut search as *mut Search as isize),
        )
    };
    search.found
}

/// Windows on another virtual desktop, and suspended UWP frames, report as
/// visible but are cloaked. Nobody can see them, so they are not on screen.
fn is_cloaked(hwnd: HWND) -> bool {
    let mut cloaked = 0u32;
    // SAFETY: the pointer and size describe `cloaked`, a u32 as the attribute
    // requires.
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut c_void,
            std::mem::size_of::<u32>() as u32,
        )
    }
    .is_ok()
        && cloaked != 0
}

/// A window's frame as a person sees it, without the invisible resize border
/// `GetWindowRect` includes. x, y, width, height.
pub fn window_bounds(hwnd: HWND) -> Option<(f64, f64, f64, f64)> {
    let mut rect = RECT::default();
    // SAFETY: the pointer and size describe `rect`, as the attribute requires.
    let framed = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut RECT as *mut c_void,
            std::mem::size_of::<RECT>() as u32,
        )
    };
    if framed.is_err() {
        // SAFETY: `rect` is a valid, writable RECT.
        unsafe { GetWindowRect(hwnd, &mut rect) }.ok()?;
    }
    Some((
        f64::from(rect.left),
        f64::from(rect.top),
        f64::from(rect.right - rect.left),
        f64::from(rect.bottom - rect.top),
    ))
}

fn enum_top_level() -> Vec<HWND> {
    unsafe extern "system" fn visit(hwnd: HWND, windows: LPARAM) -> BOOL {
        // SAFETY: `windows` is the address of the Vec below, which outlives this
        // synchronous enumeration.
        unsafe { (*(windows.0 as *mut Vec<HWND>)).push(hwnd) };
        TRUE
    }
    let mut windows: Vec<HWND> = Vec::new();
    // SAFETY: as above. EnumWindows walks front to back, which is the order
    // `window_at_point` relies on.
    let _ = unsafe { EnumWindows(Some(visit), LPARAM(&mut windows as *mut Vec<HWND> as isize)) };
    windows
}

fn layer_of(hwnd: HWND, class: &str) -> i32 {
    if SHELL_CLASSES.contains(&class) {
        return LAYER_SYSTEM;
    }
    // SAFETY: reads a style word from the window; a stale handle returns 0.
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
    if style & WS_EX_TOOLWINDOW.0 != 0 {
        LAYER_SYSTEM
    } else if style & WS_EX_TOPMOST.0 != 0 {
        LAYER_TOPMOST
    } else {
        LAYER_NORMAL
    }
}

/// One process's executable name without its extension: `notepad`, `msedge`.
pub fn process_name(pid: i32) -> Option<String> {
    let pid = u32::try_from(pid).ok()?;
    // SAFETY: plain handle open; the handle is closed below on every path.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buffer = [0u16; 1024];
    let mut length = buffer.len() as u32;
    // SAFETY: `buffer` and `length` describe a valid, writable buffer.
    let named = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    };
    // SAFETY: `process` came from OpenProcess and is closed once.
    let _ = unsafe { CloseHandle(process) };
    named.ok()?;
    let path = String::from_utf16_lossy(&buffer[..(length as usize).min(buffer.len())]);
    std::path::Path::new(&path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
}

/// Every window on screen, front to back. Minimized windows are left out: on
/// screen is what the agent is asking about, and they can't be pictured.
pub fn list_window_records() -> Vec<WindowRecord> {
    let mut names: HashMap<i32, String> = HashMap::new();
    enum_top_level()
        .into_iter()
        .filter_map(|hwnd| {
            // SAFETY: both calls only read the window's state.
            if !unsafe { IsWindowVisible(hwnd) }.as_bool() || unsafe { IsIconic(hwnd) }.as_bool() {
                return None;
            }
            if is_cloaked(hwnd) {
                return None;
            }
            let bounds = window_bounds(hwnd)?;
            if bounds.2 <= 0.0 || bounds.3 <= 0.0 {
                return None;
            }
            let class = class_name(hwnd);
            let pid = app_pid(hwnd);
            let owner = names
                .entry(pid)
                .or_insert_with(|| process_name(pid).unwrap_or_default())
                .clone();
            Some(WindowRecord {
                id: id_from_hwnd(hwnd),
                pid,
                owner,
                title: window_title(hwnd),
                bounds,
                layer: layer_of(hwnd, &class),
            })
        })
        .collect()
}

/// The window in front, as a record.
pub fn foreground_record() -> Option<WindowRecord> {
    // SAFETY: no arguments; a null handle means nothing is in front.
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        return None;
    }
    let pid = app_pid(hwnd);
    Some(WindowRecord {
        id: id_from_hwnd(hwnd),
        pid,
        owner: process_name(pid).unwrap_or_default(),
        title: window_title(hwnd),
        bounds: window_bounds(hwnd).unwrap_or_default(),
        layer: layer_of(hwnd, &class_name(hwnd)),
    })
}

pub fn foreground_window() -> Option<HWND> {
    // SAFETY: no arguments; a null handle means nothing is in front.
    let hwnd = unsafe { GetForegroundWindow() };
    (!hwnd.is_invalid()).then_some(hwnd)
}

pub fn window_exists(hwnd: HWND) -> bool {
    // SAFETY: only asks whether the handle names a window.
    unsafe { IsWindow(Some(hwnd)) }.as_bool()
}

/// The top-level window an inner window belongs to.
pub fn root_of(hwnd: HWND) -> HWND {
    // SAFETY: reads the window hierarchy; a stale handle returns null.
    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    if root.is_invalid() {
        hwnd
    } else {
        root
    }
}

/// The child window under a screen point, as deep as child windows go.
pub fn deepest_child(root: HWND, x: i32, y: i32) -> HWND {
    use windows::Win32::Graphics::Gdi::ScreenToClient;
    let mut current = root;
    for _ in 0..16 {
        let mut point = POINT { x, y };
        // SAFETY: `point` is a valid, writable POINT.
        if !unsafe { ScreenToClient(current, &mut point) }.as_bool() {
            break;
        }
        // SAFETY: reads the window hierarchy; returns null for a miss.
        let child = unsafe {
            ChildWindowFromPointEx(current, point, CWP_SKIPINVISIBLE | CWP_SKIPTRANSPARENT)
        };
        if child.is_invalid() || child == current {
            break;
        }
        current = child;
    }
    current
}

/// The inner window holding the keyboard focus inside a top-level window. This
/// works on a window that isn't in front: each thread keeps its own focus.
pub fn focused_child(root: HWND) -> HWND {
    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` is valid and writable, with cbSize set as the call requires.
    if unsafe { GetGUIThreadInfo(window_thread(root), &mut info) }.is_ok()
        && !info.hwndFocus.is_invalid()
        && root_of(info.hwndFocus) == root
    {
        return info.hwndFocus;
    }
    root
}

pub fn cursor_point() -> Option<(i32, i32)> {
    let mut point = POINT::default();
    // SAFETY: `point` is a valid, writable POINT.
    unsafe { GetCursorPos(&mut point) }.ok()?;
    Some((point.x, point.y))
}

/// Bring a window to the front, which is only ever done for the last-resort
/// physical tier or to hand the foreground back. Windows refuses
/// `SetForegroundWindow` from a background process, so when the plain call is
/// refused the calling thread joins the foreground thread's input queue, which
/// is what the rule looks at. Reports whether the window ended up in front.
pub fn activate_window(hwnd: HWND) -> bool {
    // SAFETY: each call only reads or changes window state, and a stale handle
    // makes it fail. The thread attachment is undone on the same path.
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        if SetForegroundWindow(hwnd).as_bool() && GetForegroundWindow() == hwnd {
            return true;
        }
        let ours = GetCurrentThreadId();
        let theirs = GetWindowThreadProcessId(GetForegroundWindow(), None);
        let attached =
            theirs != 0 && theirs != ours && AttachThreadInput(ours, theirs, true).as_bool();
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        if attached {
            let _ = AttachThreadInput(ours, theirs, false);
        }
        GetForegroundWindow() == hwnd
    }
}

/// The app's frontmost window on screen.
pub fn top_window_of_pid(pid: i32) -> Option<HWND> {
    list_window_records()
        .into_iter()
        .find(|w| w.pid == pid && (-1..20).contains(&w.layer))
        .map(|w| hwnd_from_id(w.id))
}

fn token_is_elevated(token: HANDLE) -> Option<bool> {
    let mut elevation = TOKEN_ELEVATION::default();
    let mut written = 0u32;
    // SAFETY: the pointer and size describe `elevation`, as the class requires.
    unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut TOKEN_ELEVATION as *mut c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut written,
        )
    }
    .ok()?;
    Some(elevation.TokenIsElevated != 0)
}

/// Whether Juno itself runs as administrator, which would put every window
/// within reach.
fn this_process_is_elevated() -> bool {
    use windows::Win32::System::Threading::GetCurrentProcess;
    let mut token = HANDLE::default();
    // SAFETY: `token` is valid and writable; the pseudo-handle needs no closing,
    // and the token is closed below.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.is_err() {
        return false;
    }
    let elevated = token_is_elevated(token).unwrap_or(false);
    // SAFETY: `token` came from OpenProcessToken and is closed once.
    let _ = unsafe { CloseHandle(token) };
    elevated
}

/// Whether a process is out of Juno's reach because it runs as administrator.
/// Windows drops input to such a window with no error at all (UIPI), so this
/// is asked first and answered out loud.
///
/// A token we may not even open is the normal shape of the answer yes: an
/// elevated token's security descriptor admits only administrators.
pub fn process_is_elevated(pid: i32) -> bool {
    let Ok(pid) = u32::try_from(pid) else {
        return false;
    };
    if pid == 0 || this_process_is_elevated() {
        return false;
    }
    // SAFETY: plain handle open; closed below on every path.
    let process = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(process) => process,
        Err(e) => return e.code() == E_ACCESSDENIED,
    };
    let mut token = HANDLE::default();
    // SAFETY: `token` is valid and writable; `process` is open.
    let opened = unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) };
    let elevated = match opened {
        Ok(()) => {
            let elevated = token_is_elevated(token).unwrap_or(false);
            // SAFETY: `token` came from OpenProcessToken and is closed once.
            let _ = unsafe { CloseHandle(token) };
            elevated
        }
        Err(e) => e.code() == E_ACCESSDENIED,
    };
    // SAFETY: `process` came from OpenProcess and is closed once.
    let _ = unsafe { CloseHandle(process) };
    elevated
}

/// Resize a window where it is, without raising it.
pub fn resize_window(window_id: u32, width: i32, height: i32) -> Result<(), AutomationError> {
    set_window_pos(window_id, 0, 0, width, height, SWP_NOMOVE)
}

/// Move a window at its size, without raising it.
pub fn move_window(window_id: u32, x: i32, y: i32) -> Result<(), AutomationError> {
    set_window_pos(window_id, x, y, 0, 0, SWP_NOSIZE)
}

fn set_window_pos(
    window_id: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    keep: SET_WINDOW_POS_FLAGS,
) -> Result<(), AutomationError> {
    // SAFETY: takes values only; a stale handle makes the call fail.
    unsafe {
        SetWindowPos(
            hwnd_from_id(window_id),
            None,
            x,
            y,
            width,
            height,
            keep | SWP_NOACTIVATE | SWP_NOZORDER,
        )
    }
    .map_err(|e| AutomationError::PlatformError(format!("Couldn't change the window: {e}")))
}

/// Ask a window to close, the way its close button does: it can still ask to
/// save. Doesn't raise it.
pub fn close_window(window_id: u32) -> Result<(), AutomationError> {
    // SAFETY: posts plain values; a stale handle only makes the call fail.
    unsafe {
        PostMessageW(
            Some(hwnd_from_id(window_id)),
            WM_CLOSE,
            WPARAM(0),
            LPARAM(0),
        )
    }
    .map_err(|e| AutomationError::PlatformError(format!("Couldn't close the window: {e}")))
}

/// One display, in physical pixels of the virtual desktop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayRecord {
    /// The monitor handle, cut to 32 bits as window ids are.
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_main: bool,
}

fn display_record(monitor: HMONITOR) -> Option<DisplayRecord> {
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` is valid and writable, with cbSize set as the call requires.
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return None;
    }
    let rect = info.rcMonitor;
    Some(DisplayRecord {
        id: monitor.0 as usize as u32,
        x: rect.left,
        y: rect.top,
        width: u32::try_from(rect.right - rect.left).unwrap_or(0),
        height: u32::try_from(rect.bottom - rect.top).unwrap_or(0),
        is_main: info.dwFlags & MONITORINFOF_PRIMARY != 0,
    })
}

/// Every active display.
pub fn list_displays() -> Vec<DisplayRecord> {
    unsafe extern "system" fn visit(
        monitor: HMONITOR,
        _dc: HDC,
        _rect: *mut RECT,
        displays: LPARAM,
    ) -> BOOL {
        if let Some(display) = display_record(monitor) {
            // SAFETY: `displays` is the address of the Vec below, which outlives
            // this synchronous enumeration.
            unsafe { (*(displays.0 as *mut Vec<DisplayRecord>)).push(display) };
        }
        TRUE
    }
    let mut displays: Vec<DisplayRecord> = Vec::new();
    // SAFETY: as above.
    let _ = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(visit),
            LPARAM(&mut displays as *mut Vec<DisplayRecord> as isize),
        )
    };
    displays
}

/// The display under a point, or the nearest one when the point is off every
/// display.
pub fn display_at_point(x: i32, y: i32) -> Option<DisplayRecord> {
    // SAFETY: takes a point by value; with this flag it always names a monitor.
    display_record(unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) })
}

/// The display under the person's cursor, which is the one a full screenshot
/// shows.
pub fn current_display() -> Option<DisplayRecord> {
    cursor_point()
        .and_then(|(x, y)| display_at_point(x, y))
        .or_else(|| list_displays().into_iter().find(|d| d.is_main))
}
