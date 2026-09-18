//! Standard save and open panels, for whole files rather than one reply.
//!
//! The share sheet already runs an `NSSavePanel` for a single response. Chat
//! export and import need the same two panels for a whole conversation, and
//! had neither: the menu items called `save_chat_export` and `load_chat_import`,
//! commands that were never written, so Export Chat and Import Chat did
//! nothing at all and said nothing about it.
//!
//! Everything AppKit runs on the main thread; the commands only dispatch.

use serde::Serialize;

/// What a panel run produced. The frontend already expects this shape.
#[derive(Debug, Default, Serialize)]
pub struct FileChoice {
    pub success: bool,
    /// Where it was written, on a save.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// What was read, on an open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl FileChoice {
    /// Someone pressed Cancel. Not an error, and nothing should be said about it.
    fn cancelled() -> Self {
        Self::default()
    }

    fn failed(error: impl Into<String>) -> Self {
        Self {
            success: false,
            error: Some(error.into()),
            ..Self::default()
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::FileChoice;
    use cocoa::base::{id, nil, YES};
    use cocoa::foundation::NSString;
    use objc::{class, msg_send, sel, sel_impl};
    use tracing::{error, info};

    const NS_MODAL_RESPONSE_OK: isize = 1;

    /// An autoreleased `NSString`.
    unsafe fn ns_string(text: &str) -> id {
        let string = NSString::alloc(nil).init_str(text);
        msg_send![string, autorelease]
    }

    /// Read an `NSString` back into Rust.
    unsafe fn rust_string(value: id) -> String {
        if value == nil {
            return String::new();
        }
        let bytes: *const std::os::raw::c_char = msg_send![value, UTF8String];
        std::ffi::CStr::from_ptr(bytes)
            .to_string_lossy()
            .into_owned()
    }

    /// SAFETY: main thread only.
    pub unsafe fn save(suggested_name: &str, contents: &str) -> FileChoice {
        let panel: id = msg_send![class!(NSSavePanel), savePanel];
        let _: () = msg_send![panel, setNameFieldStringValue: ns_string(suggested_name)];
        let _: () = msg_send![panel, setCanCreateDirectories: YES];
        let response: isize = msg_send![panel, runModal];
        if response != NS_MODAL_RESPONSE_OK {
            return FileChoice::cancelled();
        }
        let url: id = msg_send![panel, URL];
        if url == nil {
            return FileChoice::cancelled();
        }
        let path = rust_string(msg_send![url, path]);
        match std::fs::write(&path, contents) {
            Ok(()) => {
                info!("[FilePanels] Wrote {path}");
                FileChoice {
                    success: true,
                    path: Some(path),
                    ..FileChoice::default()
                }
            }
            Err(e) => {
                error!("[FilePanels] Couldn't write {path}: {e}");
                FileChoice::failed(e.to_string())
            }
        }
    }

    /// SAFETY: main thread only.
    pub unsafe fn open() -> FileChoice {
        let panel: id = msg_send![class!(NSOpenPanel), openPanel];
        let _: () = msg_send![panel, setCanChooseFiles: YES];
        let response: isize = msg_send![panel, runModal];
        if response != NS_MODAL_RESPONSE_OK {
            return FileChoice::cancelled();
        }
        let url: id = msg_send![panel, URL];
        if url == nil {
            return FileChoice::cancelled();
        }
        let path = rust_string(msg_send![url, path]);
        match std::fs::read_to_string(&path) {
            Ok(data) => FileChoice {
                success: true,
                data: Some(data),
                path: Some(path),
                ..FileChoice::default()
            },
            Err(e) => {
                error!("[FilePanels] Couldn't read {path}: {e}");
                FileChoice::failed(e.to_string())
            }
        }
    }
}

/// Write `contents` wherever the person chooses.
#[tauri::command]
pub async fn save_chat_export(app: tauri::AppHandle, data: String) -> Result<FileChoice, String> {
    run_on_main(app, move || {
        let name = format!(
            "Juno chat {}.json",
            chrono::Local::now().format("%Y-%m-%d %H.%M")
        );
        // SAFETY: dispatched to the main thread just above.
        #[cfg(target_os = "macos")]
        unsafe {
            imp::save(&name, &data)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (name, data);
            FileChoice::failed("Saving a chat is only implemented on macOS")
        }
    })
    .await
}

/// Read back a file the person chooses.
#[tauri::command]
pub async fn load_chat_import(app: tauri::AppHandle) -> Result<FileChoice, String> {
    run_on_main(app, || {
        // SAFETY: dispatched to the main thread just above.
        #[cfg(target_os = "macos")]
        unsafe {
            imp::open()
        }
        #[cfg(not(target_os = "macos"))]
        {
            FileChoice::failed("Opening a chat is only implemented on macOS")
        }
    })
    .await
}

/// Run a panel on the main thread and wait for what the person chose.
async fn run_on_main<F>(app: tauri::AppHandle, work: F) -> Result<FileChoice, String>
where
    F: FnOnce() -> FileChoice + Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        // The receiver is only dropped if the caller went away, which is not
        // worth reporting: the panel still did what it was told.
        let _ = tx.send(work());
    })
    .map_err(|e| format!("Could not show the file panel: {e}"))?;
    rx.await
        .map_err(|_| "The file panel closed without answering".to_string())
}
