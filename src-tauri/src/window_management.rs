//! # Window Management Module
//!
//! This module handles all window operations, state management, and positioning
//! for the Juno application. It provides a centralized interface for creating,
//! managing, and controlling all application windows.

use crate::constants::{self, ui::window_labels};
use tauri::utils::config::WindowConfig as DeclaredWindowConfig;
use tauri::{AppHandle, Emitter, Manager, WebviewWindowBuilder};
use tracing::{error, info, warn};

/// The label of the desktop cursor overlay window.
///
/// It has no entry in `constants::ui::window_labels` yet, and that module
/// belongs to the generated-constants pipeline, so the label lives here next to
/// its only Rust user.
pub const DESKTOP_CURSOR_OVERLAY_LABEL: &str = "desktop-cursor-overlay";

/// Find a window's declared configuration by label.
///
/// `src-tauri/tauri.conf.json` is the single source of truth for what every
/// Juno window is. Tauri parses that file once and keeps the parsed
/// [`DeclaredWindowConfig`] values on the app handle, so anything that rebuilds
/// a window at runtime can start from the exact description Tauri itself used
/// at startup.
///
/// This used to be a second, hand-maintained copy of each window's shape in
/// Rust, and the copy was always a subset. It carried size, title and
/// resizability but not `transparent`, `shadow`, `hiddenTitle`,
/// `titleBarStyle`, `skipTaskbar` or `alwaysOnTop`, so every window that went
/// through the rebuild path came back subtly wrong. For settings that meant
/// losing its transparency and therefore its vibrant sidebar the first time
/// somebody closed it with the red X. Two descriptions of one window was the
/// defect; there is now one.
fn find_declared_window<'a>(
    windows: &'a [DeclaredWindowConfig],
    label: &str,
) -> Option<&'a DeclaredWindowConfig> {
    windows.iter().find(|window| window.label == label)
}

/// The declared configuration for `label`, cloned so callers can layer runtime
/// overrides on top without editing the app's own config.
pub fn declared_window_config(app: &AppHandle, label: &str) -> Option<DeclaredWindowConfig> {
    find_declared_window(&app.config().app.windows, label).cloned()
}

/// Put a window in front of the person and make Juno the app they are in.
///
/// Three calls, in this order, because macOS treats them as three separate
/// things and the order is load-bearing:
///
/// - Showing a window only orders it into Juno's own window list. It does not
///   make Juno the active application, so a window shown while the person is
///   in another app comes up behind that app.
/// - Only focusing activates the application, and it quietly does nothing when
///   the window is hidden or minimized. So it has to come last, after the
///   other two have made it eligible.
///
/// Unminimize, then show, then focus. Focusing before unminimizing is why a
/// minimized chat window could be shown and still not come back.
fn present_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    window.unminimize().map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

/// Which chat surface a show-or-hide request should act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatSurface {
    /// The full-size chat window is on screen.
    MainWindow {
        /// Whether it is the window the person is actually looking at. A
        /// visible window that is not frontmost is the "I cannot get back to
        /// it" case, and it wants bringing forward, not putting away.
        frontmost: bool,
    },
    /// The bar is holding the conversation, either in its pane or collapsed to
    /// the idle pill. Which of those two it is, is the bar's own answer.
    BarPane,
}

/// Resolve which chat surface is active.
///
/// This reads the same fact `announce_main_window` broadcasts, the visibility
/// of the full-size window, so the tray and the bar cannot reach different
/// conclusions: Rust is the side that knows, and the bar is told. Everything
/// below the full-size window belongs to the bar, including whether its pane
/// is up, which is why that is one variant here and not two.
pub fn active_chat_surface(app: &AppHandle) -> ChatSurface {
    if !WindowManager::is_window_visible(app, window_labels::MAIN) {
        return ChatSurface::BarPane;
    }
    let frontmost = app
        .get_webview_window(window_labels::MAIN)
        .and_then(|window| window.is_focused().ok())
        .unwrap_or(false);
    ChatSurface::MainWindow { frontmost }
}

/// Show or hide a chat surface.
///
/// The single answer to "give me the chat back", used by the tray menu row and
/// by a click on the tray icon so those two cannot disagree about what is on
/// screen. It carries more weight than it sounds: Cmd+backtick only cycles
/// windows inside the frontmost app, so it can never reach Juno once focus has
/// left Juno, and with the Dock icon hidden Juno is not in Cmd+Tab either. The
/// menu bar is then the only way back, which means it has to be the dependable
/// one.
///
/// The surface is passed in rather than resolved here so the caller can decide
/// *when* to read it. The tray reads it as the pointer arrives, before AppKit
/// starts tracking a menu, because what counts is which window was in front of
/// the person when they reached for the menu bar.
pub async fn toggle_chat_surface(app: &AppHandle, surface: ChatSurface) {
    // During setup the only Juno to get back to is the setup window. Showing
    // the bar here would put it over onboarding, waiting on keys that do
    // nothing until setup ends.
    if onboarding_is_open(app) {
        bring_onboarding_forward(app);
        return;
    }
    match surface {
        // In front of the person and asked about: put it away. This goes
        // through the same command the bar's own reopen uses, so the bar hears
        // the window has gone and can take the conversation back.
        ChatSurface::MainWindow { frontmost: true } => {
            if let Err(e) = close_main_window(app.clone()).await {
                warn!("Could not put the chat window away: {}", e);
            }
        }
        // On screen, but behind something else. Asking for the chat here means
        // asking to get back to it, so bring it forward rather than hiding a
        // window the person cannot currently see.
        ChatSurface::MainWindow { frontmost: false } => {
            if let Err(e) = open_main_window(app.clone()).await {
                warn!("Could not bring the chat window forward: {}", e);
            }
        }
        // The bar owns the conversation. Put the bar itself on screen if it is
        // not (shown, never focused: it is a panel, and focusing it would pull
        // the person out of the app they are in), then let the bar flip its own
        // pane. Going through the bar's toggle rather than deciding here is
        // what lets the idle pill become the pane, which is what someone with
        // no chat on screen is asking for.
        ChatSurface::BarPane => {
            if let Some(bar) = app.get_webview_window(window_labels::FLOATING_BAR) {
                if !bar.is_visible().unwrap_or(false) {
                    if let Err(e) = bar.show() {
                        warn!("Could not put the floating bar on screen: {}", e);
                    }
                }
            }
            if let Err(e) = app.emit(constants::events::bar::TOGGLE_PANE, ()) {
                warn!("Could not ask the bar to toggle its chat pane: {}", e);
            }
        }
    }
}

/// Window management operations
pub struct WindowManager;

impl WindowManager {
    /// Show the window with this label, building it from its declared
    /// configuration if it is not currently alive.
    ///
    /// The window is described once, in `tauri.conf.json`, and this path builds
    /// from that description rather than from a copy of it. That is what makes
    /// a window recreated here indistinguishable from the one Tauri creates at
    /// startup: same transparency, same shadow, same title bar, same taskbar
    /// and always-on-top behaviour. The only thing overridden is `visible`,
    /// because the declared configs are all `visible: false` so nothing flashes
    /// on screen during launch, and "open this window" plainly means show it.
    pub async fn create_or_show_window(app: &AppHandle, label: &str) -> Result<(), String> {
        let mut config = declared_window_config(app, label).ok_or_else(|| {
            format!(
                "No window labelled '{}' is declared in tauri.conf.json",
                label
            )
        })?;

        // Check if window already exists and is valid
        if let Some(existing_window) = app.get_webview_window(label) {
            // Check if window is actually valid (not destroyed)
            match existing_window.is_visible() {
                Ok(_) => {
                    // Only steal app focus for windows whose declared config asks
                    // for it. Overlay windows (cursor overlay, floating panel,
                    // floating bar) declare `focus: false` because focusing calls
                    // [NSApp activateIgnoringOtherApps:YES], which yanks keyboard
                    // focus away from whatever app the person is actually using.
                    if config.focus {
                        present_window(&existing_window)?;
                    } else {
                        existing_window.show().map_err(|e| e.to_string())?;
                    }

                    info!("Showed existing {} window", label);
                    return Ok(());
                }
                Err(_) => {
                    // Window exists in registry but is invalid/destroyed, create new one
                    info!("Existing {} window is invalid, creating new one", label);
                }
            }
        }

        // The one runtime override: a window being opened is a window being seen.
        config.visible = true;
        // A demo build names its windows "Juno Demo".
        config.title = crate::demo::window_title(&config.title);

        let window = WebviewWindowBuilder::from_config(app, &config)
            .map_err(|e| {
                error!("Failed to prepare {} window from its config: {}", label, e);
                e.to_string()
            })?
            .build()
            .map_err(|e| {
                error!("Failed to build {} window: {}", label, e);
                e.to_string()
            })?;

        info!("Successfully built {} window", label);

        // The builder already asks for focus when the config declares it, but
        // ask again once the window exists: on macOS a window built while
        // another app is frontmost can come up behind it.
        if config.focus {
            if let Err(e) = window.set_focus() {
                warn!("Failed to set focus for {} window: {}", label, e);
            }
        }

        info!("Successfully created and showed {} window", label);
        Ok(())
    }

    /// Hide a window by label
    pub async fn hide_window(app: &AppHandle, label: &str) -> Result<(), String> {
        if let Some(window) = app.get_webview_window(label) {
            window.hide().map_err(|e| e.to_string())?;
            info!("Hidden {} window", label);
        }
        Ok(())
    }

    /// Close a window by label
    pub async fn close_window(app: &AppHandle, label: &str) -> Result<(), String> {
        if let Some(window) = app.get_webview_window(label) {
            window.close().map_err(|e| e.to_string())?;
            info!("Closed {} window", label);
        }
        Ok(())
    }

    /// Check if a window exists and is visible
    pub fn is_window_visible(app: &AppHandle, label: &str) -> bool {
        app.get_webview_window(label)
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false)
    }

    /// Focus a window by label
    pub async fn focus_window(app: &AppHandle, label: &str) -> Result<(), String> {
        if let Some(window) = app.get_webview_window(label) {
            present_window(&window)?;
            info!("Focused {} window", label);
        }
        Ok(())
    }

    /// Minimize a window by label
    pub async fn minimize_window(app: &AppHandle, label: &str) -> Result<(), String> {
        if let Some(window) = app.get_webview_window(label) {
            window.minimize().map_err(|e| e.to_string())?;
            info!("Minimized {} window", label);
        }
        Ok(())
    }

    /// Maximize/zoom a window by label
    pub async fn maximize_window(app: &AppHandle, label: &str) -> Result<(), String> {
        if let Some(window) = app.get_webview_window(label) {
            window.maximize().map_err(|e| e.to_string())?;
            info!("Maximized {} window", label);
        }
        Ok(())
    }

    /// Toggle fullscreen for a window by label
    pub async fn toggle_fullscreen(app: &AppHandle, label: &str) -> Result<(), String> {
        if let Some(window) = app.get_webview_window(label) {
            let is_fullscreen = window.is_fullscreen().map_err(|e| e.to_string())?;
            window
                .set_fullscreen(!is_fullscreen)
                .map_err(|e| e.to_string())?;
            info!(
                "Toggled fullscreen for {} window to {}",
                label, !is_fullscreen
            );
        }
        Ok(())
    }
}

/// Handle window menu events
pub async fn handle_window_menu_event(app: &AppHandle, event_id: &str) {
    match event_id {
        constants::app_menu_ids::MINIMIZE => {
            info!("[Menu] Minimize menu item clicked");
            if let Err(e) = app.emit(constants::events::menu::MINIMIZE_WINDOW_REQUESTED, ()) {
                error!("[Menu] Failed to emit minimize event: {}", e);
            }
        }
        constants::app_menu_ids::ZOOM => {
            info!("[Menu] Zoom menu item clicked");
            if let Err(e) = app.emit(constants::events::menu::ZOOM_WINDOW_REQUESTED, ()) {
                error!("[Menu] Failed to emit zoom event: {}", e);
            }
        }
        constants::app_menu_ids::BRING_ALL_TO_FRONT => {
            info!("[Menu] Bring All to Front menu item clicked");
            // This is handled automatically by macOS for most cases
            info!("[Menu] Bring All to Front executed");
        }
        constants::app_menu_ids::TOGGLE_FULLSCREEN => {
            info!("[Menu] Toggle Fullscreen menu item clicked");
            if let Err(e) = app.emit(constants::events::menu::TOGGLE_FULLSCREEN_REQUESTED, ()) {
                error!("[Menu] Failed to emit toggle fullscreen event: {}", e);
            }
        }
        _ => {
            info!("[Menu] Unhandled window menu event: {:?}", event_id);
        }
    }
}

/// Tauri command functions for window management
/// These are the command handlers that can be called from the frontend
///
/// Apply native macOS sidebar vibrancy to the settings window so its
/// translucent sidebar blurs the desktop behind it, like System Settings.
///
/// This is best-effort: any failure is logged and ignored so the window still
/// opens normally (e.g. on non-macOS builds this is compiled out entirely).
#[cfg(target_os = "macos")]
fn apply_settings_vibrancy(app: &AppHandle) {
    // `apply_vibrancy` touches AppKit and must run on the main thread; this is
    // called from an async command (a worker thread), so hop over explicitly.
    // Without this the call fails with "can only be used on the main thread"
    // and the sidebar renders as bare alpha instead of a frosted blur.
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};

        if let Some(window) = app.get_webview_window(window_labels::SETTINGS) {
            if let Err(e) = apply_vibrancy(&window, NSVisualEffectMaterial::Sidebar, None, None) {
                warn!("Failed to apply settings window vibrancy: {}", e);
            }
        }
    });
}

/// Open the native settings window.
///
/// Settings is declared at the ordinary window level, the same as the chat
/// window, and it stays there. Showing and focusing it is not enough on its
/// own: activating an application makes AppKit re-order that application's
/// windows, so settings opened while the chat window was up could come back
/// underneath it. Opening it from the floating bar, which never activates
/// Juno, made that the usual outcome rather than the occasional one.
///
/// So it is ordered above the chat window explicitly. See
/// `platform::macos::raise_above_chat_window` for why a re-order and not a
/// higher window level.
#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    WindowManager::create_or_show_window(&app, window_labels::SETTINGS).await?;

    // Give the settings window the native translucent-sidebar look. Applied
    // after the window exists/shows; idempotent across repeated opens, and
    // necessary on every open now that closing settings destroys it.
    #[cfg(target_os = "macos")]
    apply_settings_vibrancy(&app);

    crate::platform::macos::raise_above_chat_window(&app, window_labels::SETTINGS);

    // Raising settings above the chat window was never the whole fix: the
    // floating bar sits above both, at a level the chat window does not have,
    // so settings came up above the chat and still underneath the bar. The bar
    // has to be told to get out of the way, and that decision lives in one
    // place now.
    crate::bar_stacking::note_front(&app, window_labels::SETTINGS);

    Ok(())
}

/// Close the native settings window, destroying it.
///
/// Settings is destroyed rather than hidden, unlike the chat window. A hidden
/// window keeps its React tree, its listeners and its timers running for
/// something nobody is looking at, and settings in particular has to tell the
/// truth when it opens: a settings window hidden before the person granted a
/// permission in System Settings would come back still showing the old answer.
///
/// Destroying used to be the wrong trade because the rebuild path lost the
/// window's transparency and with it the vibrant sidebar. It rebuilds from the
/// declared config now, so a reopened settings window is the same window, and
/// there is nothing left to keep a hidden one alive for.
#[tauri::command]
pub async fn close_settings_window(app: AppHandle) -> Result<(), String> {
    let closed = WindowManager::close_window(&app, window_labels::SETTINGS).await;
    crate::bar_stacking::note_window_gone(&app, window_labels::SETTINGS);
    closed
}

/// Open the native onboarding window.
///
/// Setup gets the screen to itself. Every Juno window that floats above other
/// apps (the bar in whatever look it wears, the floating panel, the cursor,
/// snap-well and listening overlays) and the full-size chat window are put
/// away, because each of them looks like a working Juno while every trigger is
/// held back, and the bar is always on top so it also covers the copy. Settings
/// is closed: the person left it to come here, and a settings window kept from
/// before would come back showing permissions as they were before setup.
///
/// Onboarding is deliberately not always on top: it would cover System
/// Settings while the person is granting a permission there. Instead Juno joins
/// the Dock and Cmd+Tab while setup is open, so it can always be found, and
/// [`bring_onboarding_forward`] returns it to the front at the moments the
/// person is coming back to it.
#[tauri::command]
pub async fn open_onboarding_window(app: AppHandle) -> Result<(), String> {
    // First, before anything is on screen: the bar's own show paths check this,
    // and the window below takes a moment to build.
    ONBOARDING_ISOLATED.store(true, std::sync::atomic::Ordering::SeqCst);

    put_away_for_onboarding(&app).await;
    if let Err(e) = close_settings_window(app.clone()).await {
        warn!("Could not close settings for onboarding: {}", e);
    }
    show_in_dock_for_onboarding(&app).await;

    if let Err(e) = WindowManager::create_or_show_window(&app, window_labels::ONBOARDING).await {
        // No setup on screen means nothing may stay put away for it.
        restore_after_onboarding(&app).await;
        return Err(e);
    }
    // The bar is hidden outright for onboarding, above. This covers the race
    // where the startup timer puts it back on screen anyway: hidden or not, it
    // is not allowed to be on top of the setup window.
    crate::bar_stacking::note_front(&app, window_labels::ONBOARDING);
    Ok(())
}

/// Close the native onboarding window, putting back what opening it took away.
#[tauri::command]
pub async fn close_onboarding_window(app: AppHandle) -> Result<(), String> {
    let result = WindowManager::close_window(&app, window_labels::ONBOARDING).await;
    crate::bar_stacking::note_window_gone(&app, window_labels::ONBOARDING);
    restore_after_onboarding(&app).await;
    result
}

/// Whether onboarding has the screen, from the moment it starts opening until
/// it is closed or destroyed.
static ONBOARDING_ISOLATED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// The windows onboarding put away, so closing it only restores windows that
/// were actually going to be there.
static HIDDEN_FOR_ONBOARDING: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// True while the setup assistant is on screen, or on its way.
pub fn onboarding_is_open(app: &AppHandle) -> bool {
    ONBOARDING_ISOLATED.load(std::sync::atomic::Ordering::SeqCst)
        || WindowManager::is_window_visible(app, window_labels::ONBOARDING)
}

/// Record that the bar is being held back for onboarding, so it is put on
/// screen when setup finishes.
///
/// The bar is shown on a short timer at startup, which can land either side of
/// onboarding opening. Hiding it on open alone loses that race: the timer fires
/// afterwards and puts it straight back over the setup window. So the show path
/// checks too, and both routes mark it withheld.
pub fn mark_bar_withheld_for_onboarding() {
    remember_hidden_for_onboarding(window_labels::FLOATING_BAR);
}

fn remember_hidden_for_onboarding(label: &str) {
    let mut hidden = match HIDDEN_FOR_ONBOARDING.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if !hidden.iter().any(|l| l == label) {
        hidden.push(label.to_string());
    }
}

/// Whether a declared window is put away while onboarding is open: everything
/// that floats over other apps, and the chat window. Never onboarding itself.
fn hidden_while_onboarding(window: &DeclaredWindowConfig) -> bool {
    window.label != window_labels::ONBOARDING
        && (window.always_on_top || window.label == window_labels::MAIN)
}

/// Hide every visible Juno window onboarding should not share the screen with.
async fn put_away_for_onboarding(app: &AppHandle) {
    let labels: Vec<String> = app
        .config()
        .app
        .windows
        .iter()
        .filter(|declared| hidden_while_onboarding(declared))
        .map(|declared| declared.label.clone())
        .collect();
    for label in &labels {
        let label = label.as_str();
        if !WindowManager::is_window_visible(app, label) {
            continue;
        }
        remember_hidden_for_onboarding(label);
        if let Err(e) = WindowManager::hide_window(app, label).await {
            // Not worth failing setup over; the window merely sits in the way.
            warn!("Could not hide {} for onboarding: {}", label, e);
            continue;
        }
        if label == window_labels::MAIN {
            announce_main_window(app, false);
        }
    }
}

/// Put back what [`open_onboarding_window`] took away. Safe to call twice: the
/// close command and the window's destruction both land here.
pub async fn restore_after_onboarding(app: &AppHandle) {
    if ONBOARDING_ISOLATED.swap(false, std::sync::atomic::Ordering::SeqCst) {
        restore_dock_after_onboarding(app).await;
    }
    let hidden = {
        let mut guard = match HIDDEN_FOR_ONBOARDING.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        std::mem::take(&mut *guard)
    };
    for label in hidden {
        if label == window_labels::MAIN {
            if let Err(e) = open_main_window(app.clone()).await {
                warn!("Could not restore the chat window after onboarding: {}", e);
            }
            continue;
        }
        // Shown, never focused: these are overlays, and focusing one would
        // pull the person out of whatever they are doing.
        if let Some(window) = app.get_webview_window(&label) {
            if let Err(e) = window.show() {
                warn!("Could not restore {} after onboarding: {}", label, e);
            }
        }
    }
}

/// Put Juno in the Dock and Cmd+Tab while setup is open, if the person keeps
/// it out of them. A window that is not always on top can end up behind System
/// Settings, and a menu-bar-only app has no other way back to it.
async fn show_in_dock_for_onboarding(app: &AppHandle) {
    if !crate::commands::dock_icon::read_dock_icon_visible(app).await {
        crate::commands::dock_icon::apply_dock_icon_policy(app, true);
    }
}

/// Return the Dock icon to the person's own preference once setup is gone.
async fn restore_dock_after_onboarding(app: &AppHandle) {
    if !crate::commands::dock_icon::read_dock_icon_visible(app).await {
        crate::commands::dock_icon::apply_dock_icon_policy(app, false);
    }
}

/// Bring the setup window to the front and give it the keyboard.
///
/// Called when the person is evidently coming back to setup: a permission
/// they went to System Settings for has just been granted, they pressed their
/// trigger, or they clicked Juno in the menu bar. Does nothing when setup is
/// not on screen, and nothing while auto-grant is driving System Settings,
/// which brings the window back itself when it is done.
pub fn bring_onboarding_forward(app: &AppHandle) {
    if !WindowManager::is_window_visible(app, window_labels::ONBOARDING)
        || crate::commands::auto_grant::auto_grant_running()
    {
        return;
    }
    if let Some(window) = app.get_webview_window(window_labels::ONBOARDING) {
        if let Err(e) = present_window(&window) {
            warn!("Could not bring onboarding to the front: {}", e);
        }
    }
}

/// Open the full-size chat window, and get the bar out of its way.
///
/// The bar and this window show the same conversation, so having both up is
/// the same thing twice. The bar collapses to its idle pill rather than
/// hiding, which keeps the hotkey and the mic reachable while the big window
/// is up.
#[tauri::command]
pub async fn open_main_window(app: AppHandle) -> Result<(), String> {
    WindowManager::create_or_show_window(&app, window_labels::MAIN).await?;
    announce_main_window(&app, true);
    Ok(())
}

/// Put the full-size chat window away and give the conversation back to the bar.
///
/// Hides rather than closes: the webview keeps its React state and its scroll
/// position. This comment used to claim the window also holds the audio element
/// that plays TTS for the whole app. It does not. TTS is played by the Rust
/// backend, which spawns afplay from `crate::tts`; the HTMLAudioElement in the
/// React tree is fed by an event Rust never emits. The hide stands on the
/// React-state and bar-handover reasons alone.
#[tauri::command]
pub async fn close_main_window(app: AppHandle) -> Result<(), String> {
    WindowManager::hide_window(&app, window_labels::MAIN).await?;
    announce_main_window(&app, false);
    Ok(())
}

/// Tell the bar what the full-size window is doing.
///
/// Every route that shows the main window goes through here (the bar's button,
/// a Dock click, the tray, the reopen handler), because coordination that lives
/// in only one of them leaves the other three showing two copies of the same
/// conversation.
pub fn announce_main_window(app: &AppHandle, open: bool) {
    // The bar's place in the window stack depends on this too: the chat window
    // and the bar show the same conversation, so an idle bar belongs behind it.
    if open {
        crate::bar_stacking::note_front(app, window_labels::MAIN);
        // The bar already knows how to put its pane away.
        if let Err(e) = app.emit(constants::events::bar::DISMISS_PANE, ()) {
            warn!("Could not tell the bar to dismiss its pane: {}", e);
        }
    } else {
        crate::bar_stacking::note_window_gone(app, window_labels::MAIN);
    }
    let event = if open {
        constants::events::bar::MAIN_WINDOW_OPENED
    } else {
        constants::events::bar::MAIN_WINDOW_CLOSED
    };
    if let Err(e) = app.emit(event, ()) {
        warn!("Could not announce the main window state: {}", e);
    }
}

/// Open the desktop cursor overlay window
#[tauri::command]
pub async fn open_desktop_cursor_overlay(app: AppHandle) -> Result<(), String> {
    WindowManager::create_or_show_window(&app, DESKTOP_CURSOR_OVERLAY_LABEL).await
}

/// Get window states for tray menu and other uses
pub async fn get_window_states(app: &AppHandle) -> (bool, bool) {
    let main_visible = WindowManager::is_window_visible(app, constants::window_labels::MAIN);
    let floating_bar_visible =
        WindowManager::is_window_visible(app, constants::window_labels::FLOATING_BAR);
    (main_visible, floating_bar_visible)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real `tauri.conf.json`, parsed the same way Tauri parses it.
    ///
    /// These tests read the shipped file rather than a fixture on purpose. The
    /// bug they guard against is the declared config and the code drifting
    /// apart, and a fixture would just be a third copy to drift.
    fn declared_windows() -> Vec<DeclaredWindowConfig> {
        let raw = include_str!("../tauri.conf.json");
        let value: serde_json::Value =
            serde_json::from_str(raw).expect("tauri.conf.json is not valid JSON");
        serde_json::from_value(value["app"]["windows"].clone())
            .expect("app.windows in tauri.conf.json does not parse as Tauri window configs")
    }

    #[test]
    fn finds_a_declared_window_by_label() {
        let windows = declared_windows();

        let settings = find_declared_window(&windows, window_labels::SETTINGS)
            .expect("settings window should be declared");
        assert_eq!(settings.label, window_labels::SETTINGS);
        assert_eq!(settings.title, "Juno Settings");
        assert_eq!(settings.width, 700.0);
        assert!(settings.resizable);

        let onboarding = find_declared_window(&windows, window_labels::ONBOARDING)
            .expect("onboarding window should be declared");
        assert_eq!(onboarding.title, "Welcome to Juno");
        assert_eq!(onboarding.width, 440.0);
        assert!(!onboarding.resizable);

        let main = find_declared_window(&windows, window_labels::MAIN)
            .expect("main window should be declared");
        assert_eq!(main.title, "Juno");
    }

    #[test]
    fn an_undeclared_label_finds_nothing() {
        let windows = declared_windows();
        assert!(find_declared_window(&windows, "no-such-window").is_none());
    }

    #[test]
    fn every_window_opened_at_runtime_is_declared() {
        // create_or_show_window can only build a window it can look up, so a
        // label used in Rust but missing from tauri.conf.json is a window that
        // silently never opens. Catch it here instead of at runtime.
        let windows = declared_windows();
        for label in [
            window_labels::MAIN,
            window_labels::SETTINGS,
            window_labels::ONBOARDING,
            window_labels::FLOATING_BAR,
            DESKTOP_CURSOR_OVERLAY_LABEL,
        ] {
            assert!(
                find_declared_window(&windows, label).is_some(),
                "window '{}' is opened from Rust but not declared in tauri.conf.json",
                label
            );
        }
    }

    #[test]
    fn settings_keeps_the_properties_a_rebuild_used_to_drop() {
        // The old hand-maintained copy carried none of these, so a rebuilt
        // settings window came back opaque and lost its vibrant sidebar.
        let windows = declared_windows();
        let settings = find_declared_window(&windows, window_labels::SETTINGS)
            .expect("settings window should be declared");

        assert!(
            settings.transparent,
            "settings needs transparency for vibrancy"
        );
        assert!(settings.shadow);
        assert!(settings.hidden_title);
        assert!(!settings.skip_taskbar);
        assert!(!settings.always_on_top);
        assert!(
            settings.center,
            "settings is centred on open, and the declared config is where that is said"
        );
    }

    #[test]
    fn windows_start_hidden_so_launch_does_not_flash() {
        // Every window is declared invisible and shown deliberately, which is
        // why `visible` is the one field create_or_show_window overrides.
        let windows = declared_windows();
        for window in &windows {
            assert!(
                !window.visible,
                "window '{}' is declared visible; launch would flash it on screen",
                window.label
            );
        }
    }

    #[test]
    fn onboarding_puts_away_every_window_that_floats_and_the_chat() {
        // Lacy saw Juno on screen during setup, looking alive while every
        // trigger was held back. Anything always on top is a candidate, so the
        // rule is read off the declared config rather than a list of labels
        // that a new overlay could be missing from.
        let windows = declared_windows();
        for window in &windows {
            let hidden = hidden_while_onboarding(window);
            if window.label == window_labels::ONBOARDING {
                assert!(!hidden, "onboarding must never put itself away");
            } else if window.always_on_top || window.label == window_labels::MAIN {
                assert!(hidden, "'{}' would stay on screen over setup", window.label);
            } else {
                assert!(!hidden, "'{}' is not an overlay", window.label);
            }
        }
        for label in [
            window_labels::FLOATING_BAR,
            window_labels::FLOATING_PANEL,
            window_labels::MAIN,
            DESKTOP_CURSOR_OVERLAY_LABEL,
        ] {
            let window = find_declared_window(&windows, label).expect("declared");
            assert!(
                hidden_while_onboarding(window),
                "'{}' escaped the hide",
                label
            );
        }
    }

    #[test]
    fn onboarding_is_not_always_on_top() {
        // It would cover System Settings while a permission is being granted.
        // Juno joins the Dock instead, and comes forward when spoken to.
        let windows = declared_windows();
        let onboarding = find_declared_window(&windows, window_labels::ONBOARDING)
            .expect("onboarding window should be declared");
        assert!(!onboarding.always_on_top);
    }
}
