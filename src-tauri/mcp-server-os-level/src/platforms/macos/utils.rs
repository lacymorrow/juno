use super::wrappers::ThreadSafeAXUIElement;
use crate::AutomationError;
use accessibility::{AXAttribute, AXUIElement, AXUIElementAttributes}; // Import AXUIElementAttributes trait
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use core_foundation::base::TCFType; // Import TCFType trait
use core_foundation::string::CFString;
use core_graphics::display::{CGDisplay, CGDisplayBounds, CGMainDisplayID};
use core_graphics::image::CGImage;
use image::{ImageBuffer, ImageFormat}; // Use ImageFormat instead of ImageOutputFormat. Removed unused Pixel, RgbaImage.
use std::io::Cursor; // Added for image encoding
use tracing::debug; // Added base64 import

// Helper function to get PID from an AXUIElement
pub(crate) fn get_pid_for_element(element: &ThreadSafeAXUIElement) -> i32 {
    // Use accessibility API to get the PID
    unsafe {
        let element_ref = element.0.as_concrete_TypeRef() as *mut ::std::os::raw::c_void;

        // Use imported function
        use crate::platforms::macos::ffi::AXUIElementGetPid;

        let mut pid: i32 = 0;
        let result = AXUIElementGetPid(element_ref, &mut pid);

        if result == 0 {
            return pid;
        }

        // Fallback to -1 if we couldn't get the PID
        -1
    }
}

// Modified to return Vec<String> for multiple possible role matches
pub(crate) fn map_generic_role_to_macos_roles(role: &str) -> Vec<String> {
    match role.to_lowercase().as_str() {
        "window" => vec!["AXWindow".to_string()],
        "button" => vec![
            "AXButton".to_string(),
            "AXMenuItem".to_string(),
            "AXMenuBarItem".to_string(),
            "AXStaticText".to_string(), // Some text might be clickable buttons
            "AXImage".to_string(),      // Some images might be clickable buttons
        ], // Button can be any of these
        "checkbox" => vec!["AXCheckBox".to_string()],
        "menu" => vec!["AXMenu".to_string()],
        "menuitem" => vec!["AXMenuItem".to_string(), "AXMenuBarItem".to_string()], // Include both types
        "dialog" => vec!["AXSheet".to_string(), "AXDialog".to_string()], // macOS often uses Sheet or Dialog
        "text" | "textfield" | "input" | "textbox" => vec![
            "AXTextField".to_string(),
            "AXTextArea".to_string(),
            "AXText".to_string(),
            "AXComboBox".to_string(),
            "AXTextEdit".to_string(),
            "AXSearchField".to_string(),
            "AXWebArea".to_string(), // Web content might contain inputs
            "AXGroup".to_string(),   // Twitter uses groups that contain editable content
            "AXGenericElement".to_string(), // Generic elements that might be inputs
            "AXURIField".to_string(), // Explicit URL field type
            "AXAddressField".to_string(), // Another common name for URL fields
            "AXStaticText".to_string(), // Static text fields
        ],
        // Add specific support for URL fields
        "url" | "urlfield" => vec![
            "AXTextField".to_string(),    // URL fields are often text fields
            "AXURIField".to_string(),     // Explicit URL field type
            "AXAddressField".to_string(), // Another common name for URL fields
        ],
        "list" => vec!["AXList".to_string()],
        "listitem" => vec!["AXCell".to_string()], // List items are often cells in macOS
        "combobox" => vec!["AXPopUpButton".to_string(), "AXComboBox".to_string()],
        "tab" => vec!["AXTabGroup".to_string()],
        "tabitem" => vec!["AXRadioButton".to_string()], // Tab items are sometimes radio buttons
        "toolbar" => vec!["AXToolbar".to_string()],

        _ => vec![role.to_string()], // Keep as-is for unknown roles
    }
}

pub(crate) fn macos_role_to_generic_role(role: &str) -> Vec<String> {
    match role.to_lowercase().as_str() {
        "AXWindow" => vec!["window".to_string()],
        "AXButton" | "AXMenuItem" | "AXMenuBarItem" => vec!["button".to_string()],
        "AXTextField" | "AXTextArea" | "AXTextEdit" | "AXSearchField" | "AXURIField"
        | "AXAddressField" => vec![
            "textfield".to_string(),
            "input".to_string(),
            "textbox".to_string(),
            "url".to_string(),
            "urlfield".to_string(),
        ],
        "AXList" => vec!["list".to_string()],
        "AXCell" => vec!["listitem".to_string()],
        "AXSheet" | "AXDialog" => vec!["dialog".to_string()],
        "AXGroup" | "AXGenericElement" | "AXWebArea" => {
            vec!["group".to_string(), "genericElement".to_string()]
        }
        _ => vec![role.to_string()],
    }
}
// Helper function to get PIDs of running applications using NSWorkspace
#[allow(clippy::unexpected_cfg_condition)]
pub(crate) fn get_running_application_pids(
    use_background_apps: bool,
) -> Result<Vec<i32>, AutomationError> {
    // Implementation using Objective-C bridging
    unsafe {
        use objc::{class, msg_send, sel, sel_impl};

        let workspace_class = class!(NSWorkspace);
        let shared_workspace: *mut objc::runtime::Object =
            msg_send![workspace_class, sharedWorkspace];
        let apps: *mut objc::runtime::Object = msg_send![shared_workspace, runningApplications];
        let count: usize = msg_send![apps, count];

        let mut pids = Vec::with_capacity(count);
        for i in 0..count {
            let app: *mut objc::runtime::Object = msg_send![apps, objectAtIndex:i];

            if !use_background_apps {
                let activation_policy: i32 = msg_send![app, activationPolicy];
                // NSApplicationActivationPolicyRegular = 0
                // NSApplicationActivationPolicyAccessory = 1
                // NSApplicationActivationPolicyProhibited = 2 (background only)
                if activation_policy == 2 || activation_policy == 1 {
                    // NSApplicationActivationPolicyProhibited or NSApplicationActivationPolicyAccessory
                    continue;
                }
            }
            // Filter out common background workers by bundle identifier
            let bundle_id: *mut objc::runtime::Object = msg_send![app, bundleIdentifier];
            if !bundle_id.is_null() {
                let bundle_id_str: &str = {
                    let nsstring = bundle_id as *const objc::runtime::Object;
                    let bytes: *const std::os::raw::c_char = msg_send![nsstring, UTF8String];
                    let len: usize = msg_send![nsstring, lengthOfBytesUsingEncoding:4]; // NSUTF8StringEncoding = 4
                    let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
                    std::str::from_utf8_unchecked(bytes_slice)
                };

                // Skip common background processes and workers
                if bundle_id_str.contains(".worker")
                    || bundle_id_str.contains("com.apple.WebKit")
                    || bundle_id_str.contains("com.apple.CoreServices")
                    || bundle_id_str.contains(".helper")
                    || bundle_id_str.contains(".agent")
                {
                    debug!("Filtered out background worker: {}", bundle_id_str);
                    continue;
                }
            }

            let pid: i32 = msg_send![app, processIdentifier];
            pids.push(pid);
        }

        debug!("Found {} application PIDs", pids.len());
        Ok(pids)
    }
}

// Add this helper function after the selector handler
pub(crate) fn element_contains_text(e: &AXUIElement, text: &str) -> bool {
    // Check immediate element attributes for text
    let contains_in_value = e
        .value()
        .ok()
        .and_then(|v| v.downcast_into::<CFString>())
        .map_or(false, |s| s.to_string().contains(text));

    if contains_in_value {
        return true;
    }

    // Check title, description and other text attributes
    let contains_in_title = e
        .title()
        .ok()
        .map_or(false, |t| t.to_string().contains(text));

    let contains_in_desc = e
        .description()
        .ok()
        .map_or(false, |d| d.to_string().contains(text));

    // Check common text attributes
    for attr_name in &[
        // Changed to &[&str]
        "AXValue",
        "AXTitle",
        "AXDescription",
        "AXHelp",
        "AXLabel",
        "AXText",
    ] {
        let attr = AXAttribute::new(&CFString::new(attr_name)); // Correctly create CFString here
        if let Ok(value) = e.attribute(&attr) {
            if let Some(cf_string) = value.downcast_into::<CFString>() {
                if cf_string.to_string().contains(text) {
                    return true;
                }
            }
        }
    }

    contains_in_title || contains_in_desc
}

/// Captures a screenshot of the main display and encodes it as base64 PNG.
pub fn capture_and_encode_screenshot() -> Result<String, AutomationError> {
    let cg_image = capture_screenshot_cgimage()?; // Renamed internal function

    // Get image dimensions
    let width = cg_image.width();
    let height = cg_image.height();

    // Get raw pixel data
    let data = cg_image.data(); // Returns CFDataRef directly
    let bytes = data.bytes(); // &[u8]

    // Ensure data length matches expected size (RGBA)
    let expected_len = width * height * 4;
    if bytes.len() < expected_len {
        // Allow for extra padding bytes
        return Err(AutomationError::PlatformError(format!(
            "Screenshot data length mismatch: expected at least {}, got {}",
            expected_len,
            bytes.len()
        )));
    }

    // Create an RgbaImage from the raw bytes
    // We need to handle potential byte order issues and alpha channel position (BGRA vs RGBA)
    // Assuming macOS provides BGRA data based on common practices
    let mut img_buffer = ImageBuffer::new(width as u32, height as u32);

    for y in 0..height {
        for x in 0..width {
            let index = (y * cg_image.bytes_per_row()) + (x * 4); // Use bytes_per_row for correct indexing
            if index + 3 >= bytes.len() {
                // Avoid out-of-bounds access if data is shorter than expected for the last pixel(s)
                // This might happen with padding bytes at the end of rows.
                // We can log a warning or simply stop processing pixels.
                // For now, let's stop processing to avoid panic.
                tracing::warn!(
                    "Reached end of screenshot data prematurely at ({}, {}), index {}",
                    x,
                    y,
                    index
                );
                break; // Break inner loop
            }
            let b = bytes[index];
            let g = bytes[index + 1];
            let r = bytes[index + 2];
            let a = bytes[index + 3];
            img_buffer.put_pixel(x as u32, y as u32, image::Rgba([r, g, b, a]));
        }
        if y < height - 1
            && (y * cg_image.bytes_per_row()) + (cg_image.bytes_per_row() - 1) >= bytes.len()
        {
            // If we broke the inner loop due to reaching end of data, break outer loop too.
            tracing::warn!(
                "Stopping screenshot processing prematurely due to data length issues after row {}",
                y
            );
            break; // Break outer loop
        }
    }

    // Encode the image to PNG format in memory
    let mut png_data = Vec::new();
    {
        // Scope to ensure Cursor is dropped before reading png_data
        let mut writer = Cursor::new(&mut png_data);
        img_buffer
            .write_to(&mut writer, ImageFormat::Png)
            .map_err(|e| {
                AutomationError::Internal(format!("Failed to encode screenshot to PNG: {}", e))
            })?;
    } // Cursor is dropped here

    // Encode the PNG data to base64
    let base64_string = BASE64_STANDARD.encode(&png_data);

    Ok(base64_string)
}

/// Captures a screenshot of the main display as a CGImage. (Internal use)
fn capture_screenshot_cgimage() -> Result<CGImage, AutomationError> {
    unsafe {
        let main_display_id = CGMainDisplayID();
        let image_ref = CGDisplay::screenshot(CGDisplayBounds(main_display_id), 0, 0, 0)
            .ok_or_else(|| {
                AutomationError::PlatformError("Failed to capture screenshot".to_string())
            })?;
        Ok(image_ref)
    }
}
