//! Pictures of the screen and of one window.
//!
//! One window goes through Windows.Graphics.Capture, which draws it even when
//! other windows cover it. That API puts a yellow border around the window for
//! as long as a session is open, so a session lasts one frame. A whole display
//! goes through a GDI copy of the composited desktop, which needs no session
//! and draws no border; Juno would otherwise flash a border round the screen on
//! every turn of a computer-use run.
//!
//! Juno's own windows are left out of a display picture by excluding them from
//! capture for the length of that one copy, rather than with Tauri's
//! `contentProtected`. `contentProtected` is permanent, and the bar and its chat
//! are the same window here, so it would hide the chat from every screen share
//! as well (#836 is the reason that matters).

use super::element::ensure_com;
use super::window_list::{
    current_display, hwnd_from_id, list_displays, window_bounds, window_exists, DisplayRecord,
};
use crate::AutomationError;
use image::RgbaImage;
use std::time::{Duration, Instant};
use windows::core::{factory, IInspectable, Interface};
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, TRUE};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_BIND_FLAG,
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE,
    D3D11_MAP_READ, D3D11_RESOURCE_MISC_FLAG, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dwm::DwmFlush;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS,
    SRCCOPY,
};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowDisplayAffinity, GetWindowThreadProcessId, IsWindowVisible,
    SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WINDOW_DISPLAY_AFFINITY,
};

/// How long a window gets to deliver its first frame. Capture hands over the
/// frame it already has, so this only runs out when something is wrong.
const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(2);

fn platform_error(context: &str, e: impl std::fmt::Display) -> AutomationError {
    AutomationError::PlatformError(format!("{context}: {e}"))
}

// ── One window ──────────────────────────────────────────────────────────────

fn d3d_device_of_type(driver: D3D_DRIVER_TYPE) -> windows::core::Result<ID3D11Device> {
    let mut device = None;
    // SAFETY: `device` is a valid out-pointer; the other arguments are optional
    // and passed as None.
    unsafe {
        D3D11CreateDevice(
            None,
            driver,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )
    }?;
    device.ok_or_else(windows::core::Error::empty)
}

/// A D3D device, falling back to the software one on a machine with no usable
/// GPU (a VM, usually).
fn d3d_device() -> Result<ID3D11Device, AutomationError> {
    d3d_device_of_type(D3D_DRIVER_TYPE_HARDWARE)
        .or_else(|_| d3d_device_of_type(D3D_DRIVER_TYPE_WARP))
        .map_err(|e| platform_error("Couldn't start Direct3D for window capture", e))
}

fn winrt_device(device: &ID3D11Device) -> Result<IDirect3DDevice, AutomationError> {
    let dxgi: IDXGIDevice = device
        .cast()
        .map_err(|e| platform_error("Couldn't reach the DXGI device", e))?;
    // SAFETY: `dxgi` is a live DXGI device for the duration of the call.
    let inspectable: IInspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi) }
        .map_err(|e| platform_error("Couldn't wrap the Direct3D device", e))?;
    inspectable
        .cast()
        .map_err(|e| platform_error("Couldn't wrap the Direct3D device", e))
}

fn capture_item(hwnd: HWND) -> Result<GraphicsCaptureItem, AutomationError> {
    let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
        .map_err(|e| platform_error("Window capture isn't available", e))?;
    // SAFETY: `hwnd` names a window (checked by the caller); a stale one makes
    // the call fail.
    unsafe { interop.CreateForWindow(hwnd) }
        .map_err(|e| platform_error("Windows wouldn't capture that window", e))
}

/// Copy one captured frame out of the GPU, as RGBA.
fn frame_to_image(
    device: &ID3D11Device,
    frame: &Direct3D11CaptureFrame,
) -> Result<RgbaImage, AutomationError> {
    let surface = frame
        .Surface()
        .map_err(|e| platform_error("The captured frame has no surface", e))?;
    let access: IDirect3DDxgiInterfaceAccess = surface
        .cast()
        .map_err(|e| platform_error("Couldn't reach the captured surface", e))?;
    // SAFETY: `access` is live, and the interface asked for is the one a capture
    // surface holds.
    let texture: ID3D11Texture2D = unsafe { access.GetInterface() }
        .map_err(|e| platform_error("Couldn't reach the captured texture", e))?;

    let mut desc = D3D11_TEXTURE2D_DESC::default();
    // SAFETY: `desc` is a valid, writable description.
    unsafe { texture.GetDesc(&mut desc) };
    desc.Usage = D3D11_USAGE_STAGING;
    desc.BindFlags = D3D11_BIND_FLAG(0).0 as u32;
    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
    desc.MiscFlags = D3D11_RESOURCE_MISC_FLAG(0).0 as u32;

    let mut staging: Option<ID3D11Texture2D> = None;
    // SAFETY: `desc` is valid and `staging` is a valid out-pointer.
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut staging)) }
        .map_err(|e| platform_error("Couldn't allocate a copy of the frame", e))?;
    let staging = staging.ok_or_else(|| {
        AutomationError::PlatformError("Direct3D returned no texture".to_string())
    })?;

    // SAFETY: the context is the device's own; both textures have the same size
    // and format; the map is undone below on every path past it.
    let context: ID3D11DeviceContext = unsafe { device.GetImmediateContext() }
        .map_err(|e| platform_error("Couldn't reach the Direct3D context", e))?;
    unsafe { context.CopyResource(&staging, &texture) };
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe { context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)) }
        .map_err(|e| platform_error("Couldn't read the frame", e))?;

    let (width, height) = (desc.Width as usize, desc.Height as usize);
    let mut rgba = Vec::with_capacity(width * height * 4);
    for row in 0..height {
        // SAFETY: Map returned a buffer of RowPitch bytes per row for `height`
        // rows, each holding at least `width` BGRA pixels.
        let line = unsafe {
            std::slice::from_raw_parts(
                (mapped.pData as *const u8).add(row * mapped.RowPitch as usize),
                width * 4,
            )
        };
        for pixel in line.chunks_exact(4) {
            rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
        }
    }
    // SAFETY: mapped above with the same resource and subresource.
    unsafe { context.Unmap(&staging, 0) };

    RgbaImage::from_raw(desc.Width, desc.Height, rgba).ok_or_else(|| {
        AutomationError::Internal("The captured frame didn't match its own size".to_string())
    })
}

/// A window's frame on screen: x, y, width, height.
type WindowFrame = (f64, f64, f64, f64);

/// A picture of one window and its frame on screen, drawn even when other
/// windows cover it.
pub fn capture_window_buffer(window_id: u32) -> Result<(RgbaImage, WindowFrame), AutomationError> {
    ensure_com();
    let hwnd = hwnd_from_id(window_id);
    if !window_exists(hwnd) {
        return Err(AutomationError::ElementNotFound(format!(
            "Window {window_id} is no longer open"
        )));
    }
    let frame_on_screen = window_bounds(hwnd).ok_or_else(|| {
        AutomationError::PlatformError(format!("Couldn't read where window {window_id} is"))
    })?;
    let supported = GraphicsCaptureSession::IsSupported().unwrap_or(false);
    if !supported {
        return Err(AutomationError::UnsupportedOperation(
            "This version of Windows can't capture a single window".to_string(),
        ));
    }

    let item = capture_item(hwnd)?;
    let size = item
        .Size()
        .map_err(|e| platform_error("Couldn't read the window's size", e))?;
    let device = d3d_device()?;
    let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &winrt_device(&device)?,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        1,
        size,
    )
    .map_err(|e| platform_error("Couldn't start capturing the window", e))?;
    let session = pool
        .CreateCaptureSession(&item)
        .map_err(|e| platform_error("Couldn't start capturing the window", e))?;
    // The person's cursor isn't part of the window. Older builds lack the
    // setting and capture it anyway, which is harmless.
    let _ = session.SetIsCursorCaptureEnabled(false);
    session
        .StartCapture()
        .map_err(|e| platform_error("Couldn't start capturing the window", e))?;

    let started = Instant::now();
    let frame = loop {
        if let Ok(frame) = pool.TryGetNextFrame() {
            break Some(frame);
        }
        if started.elapsed() >= FIRST_FRAME_TIMEOUT {
            break None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let image = match &frame {
        Some(frame) => frame_to_image(&device, frame),
        None => Err(AutomationError::Timeout(format!(
            "Window {window_id} didn't deliver a picture within {} seconds",
            FIRST_FRAME_TIMEOUT.as_secs()
        ))),
    };
    if let Some(frame) = frame {
        let _ = frame.Close();
    }
    // Closing the session is what takes the yellow border away.
    let _ = session.Close();
    let _ = pool.Close();
    Ok((image?, frame_on_screen))
}

// ── A whole display ─────────────────────────────────────────────────────────

/// A window of this process, with the capture setting to put back.
type OwnWindow = (HWND, WINDOW_DISPLAY_AFFINITY);

fn own_visible_windows() -> Vec<OwnWindow> {
    unsafe extern "system" fn visit(hwnd: HWND, found: LPARAM) -> windows::core::BOOL {
        let mut pid = 0u32;
        // SAFETY: `pid` is valid and writable. `found` is the address of the Vec
        // below, which outlives this synchronous enumeration.
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == std::process::id() && IsWindowVisible(hwnd).as_bool() {
                let mut affinity = 0u32;
                let _ = GetWindowDisplayAffinity(hwnd, &mut affinity);
                (*(found.0 as *mut Vec<OwnWindow>)).push((hwnd, WINDOW_DISPLAY_AFFINITY(affinity)));
            }
        }
        TRUE
    }
    let mut found: Vec<OwnWindow> = Vec::new();
    // SAFETY: as above.
    let _ = unsafe {
        EnumWindows(
            Some(visit),
            LPARAM(&mut found as *mut Vec<OwnWindow> as isize),
        )
    };
    found
}

/// Run `capture` with Juno's windows left out of what the screen shows to a
/// capture. Windows only applies the change at the next composition, so that
/// is waited for, once on the way in.
fn without_own_windows<T>(capture: impl FnOnce() -> T) -> T {
    let windows = own_visible_windows();
    for (hwnd, _) in &windows {
        // SAFETY: each window belongs to this process, as the call requires.
        let _ = unsafe { SetWindowDisplayAffinity(*hwnd, WDA_EXCLUDEFROMCAPTURE) };
    }
    if !windows.is_empty() {
        // SAFETY: no arguments; blocks until the next composition.
        let _ = unsafe { DwmFlush() };
    }
    let result = capture();
    for (hwnd, affinity) in &windows {
        // SAFETY: as above.
        let _ = unsafe { SetWindowDisplayAffinity(*hwnd, *affinity) };
    }
    result
}

fn copy_display(display: DisplayRecord) -> Result<RgbaImage, AutomationError> {
    let (width, height) = (display.width as i32, display.height as i32);
    if width <= 0 || height <= 0 {
        return Err(AutomationError::PlatformError(
            "The display has no size".to_string(),
        ));
    }
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    // SAFETY: every GDI object is created and released here on every path. The
    // BITMAPINFO asks for 32-bit top-down rows, which `pixels` is sized for.
    let copied = unsafe {
        let screen = GetDC(None);
        let memory = CreateCompatibleDC(Some(screen));
        let bitmap = CreateCompatibleBitmap(screen, width, height);
        let previous = SelectObject(memory, bitmap.into());
        let blitted = BitBlt(
            memory,
            0,
            0,
            width,
            height,
            Some(screen),
            display.x,
            display.y,
            SRCCOPY | CAPTUREBLT,
        );
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let rows = GetDIBits(
            memory,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        );
        SelectObject(memory, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memory);
        ReleaseDC(None, screen);
        blitted.is_ok() && rows == height
    };
    if !copied {
        return Err(AutomationError::PlatformError(
            "Windows wouldn't copy the screen. It may be locked".to_string(),
        ));
    }
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
    RgbaImage::from_raw(display.width, display.height, pixels).ok_or_else(|| {
        AutomationError::Internal("The screen picture didn't match its own size".to_string())
    })
}

/// A picture of one display, by the id `list_displays` gives it.
pub fn capture_display_screenshot_buffer(display_id: u32) -> Result<RgbaImage, AutomationError> {
    let display = list_displays()
        .into_iter()
        .find(|d| d.id == display_id)
        .ok_or_else(|| {
            AutomationError::ElementNotFound(format!("Display {display_id} is no longer connected"))
        })?;
    without_own_windows(|| copy_display(display))
}

/// A picture of the display the person's cursor is on.
pub fn capture_screenshot_buffer() -> Result<RgbaImage, AutomationError> {
    let display = current_display()
        .ok_or_else(|| AutomationError::PlatformError("Windows reports no display".to_string()))?;
    without_own_windows(|| copy_display(display))
}
