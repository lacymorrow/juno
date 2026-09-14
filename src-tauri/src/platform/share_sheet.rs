//! Native share sheet for agent responses.
//!
//! `present` shows AppKit's `NSSharingServicePicker` anchored to the Share
//! button in the chat toolbar with the reply's plain text as the item, so
//! Mail, Messages, Notes, AirDrop and whatever else the person has enabled
//! show up the way they do in every other Mac app. Two Juno services are
//! appended to the list, "Save as Markdown…" and "Save as HTML…", each opening
//! a standard save panel and writing the `ResponseExport` document. Everything
//! AppKit runs on the main thread; the command only dispatches.

use crate::export::ResponseExport;
use serde::Deserialize;
use tauri::WebviewWindow;

/// The Share button's box in CSS pixels, top-left origin, relative to the web view.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ShareAnchor {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Show the share sheet for `export`, dropping down from `anchor` in `window`.
pub fn present(
    window: &WebviewWindow,
    export: ResponseExport,
    anchor: ShareAnchor,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        imp::present(window, export, anchor)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, export, anchor);
        Err("Sharing is only available on macOS".to_string())
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::ShareAnchor;
    use crate::export::{ExportFormat, ResponseExport};
    use block::ConcreteBlock;
    use cocoa::base::{id, nil, BOOL, NO, YES};
    use cocoa::foundation::{NSArray, NSPoint, NSRect, NSSize, NSString};
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::{c_char, c_void, CStr};
    use std::sync::Mutex;
    use tauri::{Manager, WebviewWindow};
    use tracing::{error, info, warn};

    /// `NSMinYEdge`: the picker drops down from the bottom of the anchor.
    const NS_MIN_Y_EDGE: usize = 1;
    const NS_MODAL_RESPONSE_OK: isize = 1;
    const DELEGATE_CLASS_NAME: &str = "JunoSharePickerDelegate";

    /// The picker and delegate for the sheet currently on screen. AppKit needs
    /// the picker alive while its menu is up, so they are held here until the
    /// next sheet replaces them. Main thread only; the `Send` impl exists so
    /// they can sit in a static.
    struct Live {
        picker: id,
        delegate: id,
    }
    unsafe impl Send for Live {}

    static LIVE: Mutex<Option<Live>> = Mutex::new(None);
    /// The document behind the sheet on screen, read back when AppKit asks the
    /// delegate for services (that callback carries no Rust state of its own).
    static PENDING: Mutex<Option<ResponseExport>> = Mutex::new(None);

    pub fn present(
        window: &WebviewWindow,
        export: ResponseExport,
        anchor: ShareAnchor,
    ) -> Result<(), String> {
        let app = window.app_handle().clone();
        let window = window.clone();
        app.run_on_main_thread(move || show(&window, export, anchor))
            .map_err(|e| format!("Couldn't reach the main thread to open the share sheet: {e}"))
    }

    fn show(window: &WebviewWindow, export: ResponseExport, anchor: ShareAnchor) {
        let text = export.plain_text();
        match PENDING.lock() {
            Ok(mut guard) => *guard = Some(export),
            Err(poisoned) => *poisoned.into_inner() = Some(export),
        }
        let view = match window.ns_view() {
            Ok(view) => view as id,
            Err(e) => {
                error!(
                    "[ShareSheet] No content view for window '{}': {e}",
                    window.label()
                );
                return;
            }
        };
        let delegate = match delegate_instance() {
            Some(delegate) => delegate,
            None => return,
        };

        // SAFETY: main thread; `view` is the window's live content view and
        // every selector below is a documented AppKit/Foundation method.
        unsafe {
            let bounds: NSRect = msg_send![view, bounds];
            let flipped: BOOL = msg_send![view, isFlipped];
            // DOM rects are top-down; an unflipped NSView counts y from the bottom.
            let y = if flipped != NO {
                anchor.y
            } else {
                bounds.size.height - anchor.y - anchor.height
            };
            let rect = NSRect::new(
                NSPoint::new(anchor.x, y),
                NSSize::new(anchor.width, anchor.height),
            );

            let item = ns_string(&text);
            let items = NSArray::arrayWithObject(nil, item);
            let picker: id = msg_send![class!(NSSharingServicePicker), alloc];
            let picker: id = msg_send![picker, initWithItems: items];
            if picker == nil {
                error!("[ShareSheet] NSSharingServicePicker refused to initialise");
                return;
            }
            let _: () = msg_send![picker, setDelegate: delegate];

            let previous = match LIVE.lock() {
                Ok(mut guard) => guard.replace(Live { picker, delegate }),
                Err(poisoned) => poisoned.into_inner().replace(Live { picker, delegate }),
            };
            if let Some(previous) = previous {
                let _: () = msg_send![previous.picker, release];
                let _: () = msg_send![previous.delegate, release];
            }

            info!(
                "[ShareSheet] Showing share sheet for {} chars",
                text.chars().count()
            );
            let _: () = msg_send![picker, showRelativeToRect: rect ofView: view preferredEdge: NS_MIN_Y_EDGE];
        }
    }

    /// A fresh delegate that appends Juno's save services to AppKit's list.
    fn delegate_instance() -> Option<id> {
        let class = match Class::get(DELEGATE_CLASS_NAME) {
            Some(class) => class,
            None => declare_delegate_class()?,
        };
        // SAFETY: `class` is a registered NSObject subclass; `new` returns a +1 object.
        let delegate: id = unsafe { msg_send![class, new] };
        if delegate == nil {
            error!("[ShareSheet] Couldn't instantiate {DELEGATE_CLASS_NAME}");
            return None;
        }
        Some(delegate)
    }

    fn declare_delegate_class() -> Option<&'static Class> {
        #[allow(unexpected_cfgs)] // cfg from the class! macro
        let superclass = class!(NSObject);
        let mut decl = match ClassDecl::new(DELEGATE_CLASS_NAME, superclass) {
            Some(decl) => decl,
            None => {
                error!("[ShareSheet] Couldn't declare {DELEGATE_CLASS_NAME}");
                return None;
            }
        };
        // SAFETY: the signature matches the NSSharingServicePickerDelegate method.
        unsafe {
            #[allow(unexpected_cfgs)] // cfg from the sel! macro
            decl.add_method(
                sel!(sharingServicePicker:sharingServicesForItems:proposedSharingServices:),
                services_for_items as extern "C" fn(&Object, Sel, id, id, id) -> id,
            );
        }
        Some(decl.register())
    }

    /// `sharingServicePicker:sharingServicesForItems:proposedSharingServices:`:
    /// AppKit's own services first, then Juno's two save options.
    extern "C" fn services_for_items(
        _this: &Object,
        _sel: Sel,
        _picker: id,
        items: id,
        proposed: id,
    ) -> id {
        // SAFETY: AppKit calls this on the main thread with live objects; the
        // returned array is autoreleased as the delegate contract expects.
        let export = match PENDING.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        let Some(export) = export else {
            warn!("[ShareSheet] Delegate asked for services with no document pending");
            return proposed;
        };
        let _ = items;
        unsafe {
            let services: id = msg_send![proposed, mutableCopy];
            let services: id = msg_send![services, autorelease];
            let extras = [
                ("Save as Markdown…", "doc.plaintext", ExportFormat::Markdown),
                (
                    "Save as HTML…",
                    "chevron.left.forwardslash.chevron.right",
                    ExportFormat::Html,
                ),
            ];
            for (title, symbol, format) in extras {
                let service = custom_service(title, symbol, export.clone(), format);
                if service != nil {
                    let _: () = msg_send![services, addObject: service];
                }
            }
            services
        }
    }

    /// An `NSSharingService` with a Rust handler. Autoreleased.
    unsafe fn custom_service(
        title: &str,
        symbol: &str,
        export: ResponseExport,
        format: ExportFormat,
    ) -> id {
        let image: id = msg_send![
            class!(NSImage),
            imageWithSystemSymbolName: ns_string(symbol)
            accessibilityDescription: nil
        ];
        let handler = ConcreteBlock::new(move || save_with_panel(&export, format)).copy();
        let service: id = msg_send![class!(NSSharingService), alloc];
        // AppKit copies the handler block, so the RcBlock may drop after this call.
        let service: id = msg_send![
            service,
            initWithTitle: ns_string(title)
            image: image
            alternateImage: nil
            handler: &*handler as *const _ as *const c_void
        ];
        if service == nil {
            warn!("[ShareSheet] NSSharingService for '{title}' refused to initialise");
            return nil;
        }
        msg_send![service, autorelease]
    }

    /// Run the standard save panel and write the export where the person chose.
    fn save_with_panel(export: &ResponseExport, format: ExportFormat) {
        let file_name = export.file_name(format);
        // SAFETY: called on the main thread from the sharing service handler.
        let path = unsafe {
            let panel: id = msg_send![class!(NSSavePanel), savePanel];
            let _: () = msg_send![panel, setNameFieldStringValue: ns_string(&file_name)];
            let _: () = msg_send![panel, setCanCreateDirectories: YES];
            let _: () = msg_send![panel, setExtensionHidden: NO];
            let response: isize = msg_send![panel, runModal];
            if response != NS_MODAL_RESPONSE_OK {
                return;
            }
            let url: id = msg_send![panel, URL];
            if url == nil {
                return;
            }
            let path: id = msg_send![url, path];
            rust_string(path)
        };

        match std::fs::write(&path, export.render(format)) {
            Ok(()) => info!(
                "[ShareSheet] Saved response as {} to {path}",
                format.label()
            ),
            Err(e) => {
                error!("[ShareSheet] Couldn't write {path}: {e}");
                // SAFETY: main thread; NSAlert is the standard way to report this.
                unsafe {
                    let alert: id = msg_send![class!(NSAlert), new];
                    let alert: id = msg_send![alert, autorelease];
                    let _: () =
                        msg_send![alert, setMessageText: ns_string("Couldn't save the response")];
                    let _: () = msg_send![alert, setInformativeText: ns_string(&e.to_string())];
                    let _: isize = msg_send![alert, runModal];
                }
            }
        }
    }

    /// An autoreleased `NSString`.
    unsafe fn ns_string(text: &str) -> id {
        let string = NSString::alloc(nil).init_str(text);
        msg_send![string, autorelease]
    }

    unsafe fn rust_string(string: id) -> String {
        if string == nil {
            return String::new();
        }
        let bytes: *const c_char = msg_send![string, UTF8String];
        if bytes.is_null() {
            return String::new();
        }
        CStr::from_ptr(bytes).to_string_lossy().into_owned()
    }
}
