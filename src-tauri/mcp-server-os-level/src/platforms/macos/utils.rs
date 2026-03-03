use accessibility::{AXAttribute, AXUIElement, AXUIElementAttributes}; // Import AXUIElementAttributes trait
use base64::prelude::{Engine as _, BASE64_STANDARD};
use core_foundation::string::CFString;
use core_graphics::display::{CGDisplay, CGDisplayBounds, CGMainDisplayID, CGGetActiveDisplayList, CGDirectDisplayID}; // Use CGGetActiveDisplayList
use core_graphics::geometry::{CGRect, CGPoint}; // Removed CGPointMake, CGRectContainsPoint
use core_graphics::image::CGImage;
use image::{ImageBuffer, ImageFormat, Rgba, imageops}; // Use ImageFormat instead of ImageOutputFormat. Removed unused Pixel, RgbaImage. Added imageops
use std::io::Cursor; // Added for image encoding
use tracing::{debug, warn}; // Added warn
use super::element::MacOSUIElement; // Added for the new function
use crate::element::UIElementImpl; // Import the trait providing .attributes()
use core_graphics::event::{CGEvent}; // From HEAD
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID}; // From HEAD
use crate::AutomationError; // From Main

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
        "axwindow" => vec!["window".to_string()],
        "axbutton" | "axmenuitem" | "axmenubaritem" => vec!["button".to_string()],
        "axtextfield" | "axtextarea" | "axtextedit" | "axsearchfield" | "axurifield"
        | "axaddressfield" => vec![
            "textfield".to_string(),
            "input".to_string(),
            "textbox".to_string(),
            "url".to_string(),
            "urlfield".to_string(),
        ],
        "axlist" => vec!["list".to_string()],
        "axcell" => vec!["listitem".to_string()],
        "axsheet" | "axdialog" => vec!["dialog".to_string()],
        "axgroup" | "axgenericelement" | "axwebarea" => {
            vec!["group".to_string(), "genericElement".to_string()]
        }
        _ => vec![role.to_string()],
    }
}
// Helper function to get PIDs of running applications using NSWorkspace
// #[allow(clippy::unexpected_cfg_condition)] // Removed: deprecated cfg condition
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
                let bundle_id_str: String = {
                    let nsstring = bundle_id as *const objc::runtime::Object;
                    let bytes: *const std::os::raw::c_char = msg_send![nsstring, UTF8String];
                    let len: usize = msg_send![nsstring, lengthOfBytesUsingEncoding:4]; // NSUTF8StringEncoding = 4
                    let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
                    String::from_utf8_lossy(bytes_slice).into_owned()
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
        .is_some_and(|s| s.to_string().contains(text));

    if contains_in_value {
        return true;
    }

    // Check title, description and other text attributes
    let contains_in_title = e
        .title()
        .ok()
        .is_some_and(|t| t.to_string().contains(text));

    let contains_in_desc = e
        .description()
        .ok()
        .is_some_and(|d| d.to_string().contains(text));

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
    // 1. Get current cursor position
    let cursor_point = {
        // Use kCGEventSourceStateHIDSystemState to get the event source for system events
        let event_source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| AutomationError::PlatformError("Failed to create HID event source".to_string()))?;
        let event = CGEvent::new(event_source).map_err(|_| {
            AutomationError::PlatformError("Failed to create null CGEvent to get location".to_string())
        })?;
        event.location() // This returns CGPoint in global coordinates
    };
    debug!("Current cursor position: ({}, {})", cursor_point.x, cursor_point.y);

    // 2. Find the display containing the cursor
    let target_display_id = match find_display_containing_point(cursor_point) {
        Ok(id) => {
            debug!("Cursor found on display ID: {}", id);
            id
        },
        Err(e) => {
            warn!("Failed to find display for cursor at ({}, {}): {}. Falling back to main display.", cursor_point.x, cursor_point.y, e);
            unsafe { CGMainDisplayID() } // Fallback to main display
        }
    };

    // 3. Capture the specific display (uses ScreenCaptureKit when available)
    let buffer = capture_display_buffer(Some(target_display_id))?;
    debug!("Captured screenshot for display ID: {}", target_display_id);

    // 4. Encode
    encode_imagebuffer_to_base64_png(&buffer)
}

/// Captures a screenshot of a specific UI element and encodes it as base64 PNG.
/// Currently assumes the element is on the main display.
pub fn capture_element_screenshot(element: &MacOSUIElement) -> Result<String, AutomationError> {
    let (x, y, width, height) = element.bounds()?;

    // Add check for zero or negative dimensions immediately after getting bounds
    // Check if dimensions are strictly positive before proceeding
    if width <= 0.0 || height <= 0.0 {
        let attrs = element.attributes(); // Get attributes for context
        let role = attrs.role;
        let label = attrs.label.unwrap_or_else(|| "N/A".to_string());
        // Log the specific warning
        warn!(
            "Cannot capture screenshot for element with zero or negative dimensions. Role: '{}', Label: '{}', Bounds: ({}, {}, {}, {})",
            role, label, x, y, width, height
        );
        // Return the specific error variant
        return Err(AutomationError::ZeroElementDimensions {
            role,
            label,
            x,
            y,
            width,
            height,
        });
    }

    // Get the center point of the element
    let center_x = x + width / 2.0;
    let center_y = y + height / 2.0;
    let element_center_point = CGPoint { x: center_x, y: center_y }; // Construct CGPoint directly

    // Find the display containing the element's center point
    let target_display_id = match find_display_containing_point(element_center_point) {
        Ok(id) => id,
        Err(_) => {
            warn!(
                "Could not determine display for element at ({}, {}). Defaulting to main display.",
                center_x, center_y
            );
            unsafe { CGMainDisplayID() } // Fallback to main display
        }
    };

    // Get the bounds of the target display
    let display_bounds = unsafe { CGDisplayBounds(target_display_id) };

    // Adjust element coordinates to be relative to the target display's origin
    let relative_x = x - display_bounds.origin.x;
    let relative_y = y - display_bounds.origin.y;

    // Create a CGRect for the element's bounds, now relative to the target display
    // Ensure bounds are positive and convert to u32 for cropping
    // Use floor() for position and ceil() for dimensions, ensuring minimum 1px size.
    let crop_x = relative_x.max(0.0).floor() as u32;
    let crop_y = relative_y.max(0.0).floor() as u32;
    let crop_width = width.max(1.0).ceil() as u32; // Ensure at least 1px width
    let crop_height = height.max(1.0).ceil() as u32; // Ensure at least 1px height

    // Capture the *target* display (uses ScreenCaptureKit when available)
    let display_buffer = capture_display_buffer(Some(target_display_id))?;

    // Crop the ImageBuffer
    // Check if crop dimensions are valid within the display buffer
    if crop_x + crop_width > display_buffer.width() || crop_y + crop_height > display_buffer.height() {
        warn!(
            "Element bounds ({}, {}, {}, {}) exceed screen dimensions ({}, {}). Clamping crop.",
            crop_x, crop_y, crop_width, crop_height, display_buffer.width(), display_buffer.height()
        );
        // Optionally clamp dimensions, or return error. For now, let crop_imm handle it (it might panic or return subimage).
        // Let's clamp to avoid panic from crop_imm if width/height are 0 or exceed boundaries after clamping x/y.
        let clamped_width = crop_width.min(display_buffer.width().saturating_sub(crop_x));
        let clamped_height = crop_height.min(display_buffer.height().saturating_sub(crop_y));
         if clamped_width == 0 || clamped_height == 0 {
             // Add element context to this error message as well
             let attrs = element.attributes();
             let role = attrs.role;
             let label = attrs.label.unwrap_or_else(|| "N/A".to_string());
             let err_msg = format!(
                "Element bounds result in zero-size crop area after clamping. Role: '{}', Label: '{}', Original Bounds: ({}, {}, {}, {}), Clamped Crop: ({}, {}, {}, {})",
                role, label, x, y, width, height, crop_x, crop_y, clamped_width, clamped_height
             );
             warn!("{}", err_msg);
            // Keep this as PlatformError for now, as it's a different condition (clamping issue)
            return Err(AutomationError::PlatformError(err_msg));
        }
        let cropped_buffer = imageops::crop_imm(
            &display_buffer,
            crop_x,
            crop_y,
            clamped_width,
            clamped_height,
        ).to_image();
         encode_imagebuffer_to_base64_png(&cropped_buffer) // Encode the cropped buffer

    } else if crop_width == 0 || crop_height == 0 {
         // Also add context here for the direct zero-size crop case
         let attrs = element.attributes();
         let role = attrs.role;
         let label = attrs.label.unwrap_or_else(|| "N/A".to_string());
         let err_msg = format!(
            "Element bounds result in zero-size crop area before clamping (likely due to u32 conversion). Role: '{}', Label: '{}', Original Bounds: ({}, {}, {}, {})",
            role, label, x, y, width, height
         );
         warn!("{}", err_msg);
         // Keep this as PlatformError as well (conversion/clamping issue)
         Err(AutomationError::PlatformError(err_msg))
    } else {
        // Crop the buffer using the original element bounds (as u32)
        let cropped_buffer = imageops::crop_imm(
            &display_buffer,
            crop_x,
            crop_y,
            crop_width,
            crop_height
        ).to_image();
         encode_imagebuffer_to_base64_png(&cropped_buffer) // Encode the cropped buffer
    }
}

/// Captures a screenshot of a specific window and encodes it as base64 PNG.
/// `window_element` should be a UIElement representing a window (role == "AXWindow").
pub fn capture_window_screenshot(window_element: &MacOSUIElement) -> Result<String, AutomationError> {
    // Verify that the element is a window
    let attrs = window_element.attributes();
    if attrs.role != "AXWindow" {
        return Err(AutomationError::PlatformError(format!(
            "Element is not a window. Expected role 'AXWindow', got '{}'",
            attrs.role
        )));
    }

    // Get window bounds
    let (x, y, width, height) = window_element.bounds()?;

    // Check for valid dimensions
    if width <= 0.0 || height <= 0.0 {
        let label = attrs.label.unwrap_or_else(|| "N/A".to_string());
        warn!(
            "Cannot capture screenshot for window with zero or negative dimensions. Label: '{}', Bounds: ({}, {}, {}, {})",
            label, x, y, width, height
        );
        return Err(AutomationError::ZeroElementDimensions {
            role: attrs.role,
            label,
            x,
            y,
            width,
            height,
        });
    }

    // Get the center point of the window to determine which display it's on
    let center_x = x + width / 2.0;
    let center_y = y + height / 2.0;
    let window_center_point = CGPoint { x: center_x, y: center_y };

    // Find the display containing the window's center point
    let target_display_id = match find_display_containing_point(window_center_point) {
        Ok(id) => id,
        Err(_) => {
            warn!(
                "Could not determine display for window at ({}, {}). Defaulting to main display.",
                center_x, center_y
            );
            unsafe { CGMainDisplayID() }
        }
    };

    // Get the bounds of the target display
    let display_bounds = unsafe { CGDisplayBounds(target_display_id) };

    // Adjust window coordinates to be relative to the target display's origin
    let relative_x = x - display_bounds.origin.x;
    let relative_y = y - display_bounds.origin.y;

    // Create crop parameters, ensuring they're positive and within display bounds
    let crop_x = relative_x.max(0.0).floor() as u32;
    let crop_y = relative_y.max(0.0).floor() as u32;
    let crop_width = width.max(1.0).ceil() as u32;
    let crop_height = height.max(1.0).ceil() as u32;

    // Capture the target display (uses ScreenCaptureKit when available)
    let display_buffer = capture_display_buffer(Some(target_display_id))?;

    // Validate crop dimensions
    if crop_x + crop_width > display_buffer.width() || crop_y + crop_height > display_buffer.height() {
        warn!(
            "Window bounds ({}, {}, {}, {}) exceed screen dimensions ({}, {}). Clamping crop.",
            crop_x, crop_y, crop_width, crop_height, display_buffer.width(), display_buffer.height()
        );

        let clamped_width = crop_width.min(display_buffer.width().saturating_sub(crop_x));
        let clamped_height = crop_height.min(display_buffer.height().saturating_sub(crop_y));

        if clamped_width == 0 || clamped_height == 0 {
            let label = attrs.label.unwrap_or_else(|| "N/A".to_string());
            let err_msg = format!(
                "Window bounds result in zero-size crop area after clamping. Label: '{}', Original Bounds: ({}, {}, {}, {}), Clamped Crop: ({}, {}, {}, {})",
                label, x, y, width, height, crop_x, crop_y, clamped_width, clamped_height
            );
            warn!("{}", err_msg);
            return Err(AutomationError::PlatformError(err_msg));
        }

        let cropped_buffer = imageops::crop_imm(
            &display_buffer,
            crop_x,
            crop_y,
            clamped_width,
            clamped_height,
        ).to_image();
        encode_imagebuffer_to_base64_png(&cropped_buffer)
    } else if crop_width == 0 || crop_height == 0 {
        let label = attrs.label.unwrap_or_else(|| "N/A".to_string());
        let err_msg = format!(
            "Window bounds result in zero-size crop area. Label: '{}', Original Bounds: ({}, {}, {}, {})",
            label, x, y, width, height
        );
        warn!("{}", err_msg);
        Err(AutomationError::PlatformError(err_msg))
    } else {
        // Crop the buffer using the window bounds
        let cropped_buffer = imageops::crop_imm(
            &display_buffer,
            crop_x,
            crop_y,
            crop_width,
            crop_height
        ).to_image();
        encode_imagebuffer_to_base64_png(&cropped_buffer)
    }
}

/// Converts a CGImage into an ImageBuffer<Rgba<u8>, Vec<u8>>.
fn cgimage_to_imagebuffer(cg_image: CGImage) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, AutomationError> {
    let width = cg_image.width();
    let height = cg_image.height();
    let data = cg_image.data();
    let bytes = data.bytes();

    let expected_len_min = width * height * 4;
    if bytes.len() < expected_len_min {
        return Err(AutomationError::PlatformError(format!(
            "Screenshot data length mismatch: expected at least {}, got {}",
            expected_len_min,
            bytes.len()
        )));
    }

    let mut img_buffer = ImageBuffer::new(width as u32, height as u32);
    for y in 0..height {
        for x in 0..width {
            let index = (y * cg_image.bytes_per_row()) + (x * 4);
            if index + 3 >= bytes.len() {
                warn!(
                    "Reached end of screenshot data prematurely at ({}, {}), index {}",
                    x, y, index
                );
                break;
            }
            let b = bytes[index];
            let g = bytes[index + 1];
            let r = bytes[index + 2];
            let a = bytes[index + 3];
            img_buffer.put_pixel(x as u32, y as u32, Rgba([r, g, b, a]));
        }
    }
    Ok(img_buffer)
}

/// Encodes an ImageBuffer into a base64 PNG string.
fn encode_imagebuffer_to_base64_png(buffer: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Result<String, AutomationError> {
    let mut png_data = Cursor::new(Vec::new());
    buffer
        .write_to(&mut png_data, ImageFormat::Png)
        .map_err(|e| AutomationError::PlatformError(format!("Failed to encode PNG: {}", e)))?;
    let base64_string = BASE64_STANDARD.encode(png_data.into_inner());
    Ok(base64_string)
}

/// Captures a screenshot of the specified display using ScreenCaptureKit (macOS 14.0+).
/// Returns raw RGBA pixel data as an ImageBuffer, which is faster and more efficient
/// than the legacy CGDisplay::screenshot() path.
#[cfg(feature = "screencapturekit-backend")]
fn capture_via_screencapturekit(display_id: Option<CGDirectDisplayID>) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, AutomationError> {
    use screencapturekit::screenshot_manager::SCScreenshotManager;
    use screencapturekit::shareable_content::SCShareableContent;
    use screencapturekit::stream::content_filter::SCContentFilter;
    use screencapturekit::stream::configuration::SCStreamConfiguration;

    // Get shareable content (enumerates displays/windows)
    let content = SCShareableContent::get()
        .map_err(|e| AutomationError::PlatformError(format!("SCShareableContent::get failed: {}", e)))?;

    let displays = content.displays();

    // Find the target display by CGDirectDisplayID
    let target_display = if let Some(id) = display_id {
        displays.iter().find(|d| d.display_id() == id)
    } else {
        displays.first()
    }.ok_or_else(|| AutomationError::PlatformError("No displays found via ScreenCaptureKit".to_string()))?;

    debug!("ScreenCaptureKit: capturing display {} ({}x{})",
        target_display.display_id(), target_display.width(), target_display.height());

    // Create content filter for this display (capture everything, exclude nothing)
    let filter = SCContentFilter::create()
        .with_display(target_display)
        .with_excluding_windows(&[])
        .build();

    // Use default configuration — captures at native resolution
    let config = SCStreamConfiguration::new();

    // Capture single screenshot (synchronous — blocks until complete)
    let image = SCScreenshotManager::capture_image(&filter, &config)
        .map_err(|e| AutomationError::PlatformError(format!("SCScreenshotManager capture failed: {}", e)))?;

    let width = image.width();
    let height = image.height();
    debug!("ScreenCaptureKit: captured {}x{} image", width, height);

    // Get raw RGBA pixel data directly from SCK's CGImage
    let rgba_data = image.rgba_data()
        .map_err(|e| AutomationError::PlatformError(format!("Failed to get RGBA data from SCK image: {}", e)))?;

    let data_len = rgba_data.len();

    // Convert raw RGBA data to ImageBuffer
    ImageBuffer::from_raw(width as u32, height as u32, rgba_data)
        .ok_or_else(|| AutomationError::PlatformError(
            format!("Failed to create ImageBuffer from SCK data: dimensions {}x{}, data length {} (expected {})",
                width, height, data_len, width * height * 4)
        ))
}

/// Captures a screenshot of the specified display and returns it as an ImageBuffer.
///
/// Uses ScreenCaptureKit (macOS 14.0+) when available for better performance,
/// falling back to the legacy CGDisplay::screenshot() path on older macOS versions.
fn capture_display_buffer(display_id: Option<CGDirectDisplayID>) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, AutomationError> {
    // Try ScreenCaptureKit first (macOS 14.0+, 8x faster, 50% less CPU)
    #[cfg(feature = "screencapturekit-backend")]
    {
        match capture_via_screencapturekit(display_id) {
            Ok(buffer) => {
                debug!("Screenshot captured via ScreenCaptureKit");
                return Ok(buffer);
            }
            Err(e) => {
                warn!("ScreenCaptureKit capture failed, falling back to CoreGraphics: {}", e);
            }
        }
    }

    // Legacy fallback: CGDisplay::screenshot (deprecated in macOS 15.0)
    let cg_image = capture_screenshot_cgimage_legacy(display_id)?;
    cgimage_to_imagebuffer(cg_image)
}

/// Legacy screenshot capture using CGDisplay::screenshot.
/// Deprecated in macOS 15.0 — prefer ScreenCaptureKit via `capture_display_buffer()`.
fn capture_screenshot_cgimage_legacy(display_id: Option<CGDirectDisplayID>) -> Result<CGImage, AutomationError> {
    unsafe {
        let target_display_id = display_id.unwrap_or_else(|| {
            warn!("capture_screenshot_cgimage_legacy called with None display_id, defaulting to main display.");
            CGMainDisplayID()
        });
        let cg_image = CGDisplay::screenshot(CGDisplayBounds(target_display_id), 0, 0, 0)
            .ok_or_else(|| {
                AutomationError::PlatformError(format!("Failed to capture screenshot for display ID {}", target_display_id))
            })?;
        Ok(cg_image)
    }
}

/// Finds the CGDirectDisplayID of the display containing the given point (in global coordinates).
fn find_display_containing_point(point: CGPoint) -> Result<CGDirectDisplayID, AutomationError> {
    unsafe {
        const MAX_DISPLAYS: u32 = 16; // Assume a reasonable maximum number of displays
        let mut online_displays = [0; MAX_DISPLAYS as usize];
        let mut display_count: u32 = 0;

        // Get the list of online displays
        let result = CGGetActiveDisplayList(MAX_DISPLAYS, online_displays.as_mut_ptr(), &mut display_count);
        if result != 0 { // Check for errors (kCGErrorSuccess is 0)
            return Err(AutomationError::PlatformError(format!("Failed to get online display list: error code {}", result)));
        }

        if display_count == 0 {
            return Err(AutomationError::PlatformError("No active displays found".to_string()));
        }

        // Iterate through the displays and check if the point is within their bounds
        for i in 0..display_count {
            let display_id = online_displays[i as usize];
            let bounds: CGRect = CGDisplayBounds(display_id);
            if bounds.contains(&point) {
                debug!("Point ({}, {}) found on display {} with bounds {:?}", point.x, point.y, display_id, bounds);
                return Ok(display_id);
            }
        }

        // If no display contains the point (e.g., point is in the bezel space?)
        // Fallback: return the main display ID
        warn!("Point ({}, {}) not found within any display bounds. Falling back to main display.", point.x, point.y);
        Ok(CGMainDisplayID())
        // Or return an error if preferred:
        // Err(AutomationError::PlatformError(format!("Point ({}, {}) not found on any active display", point.x, point.y)))
    }
}

/// Checks if the current process has accessibility permissions.
pub fn check_accessibility_permissions() -> bool {
    unsafe {
        // Call the FFI function. Passing NULL (as CFDictionaryRef which is a pointer)
        // for options dictionary defaults to checking standard accessibility trust.
        use super::ffi::AXIsProcessTrustedWithOptions;
        AXIsProcessTrustedWithOptions(std::ptr::null())
    }
}

pub fn get_display_bounds(display_id: Option<CGDirectDisplayID>) -> Result<CGRect, AutomationError> {
    unsafe {
        // Use unwrap_or_else with a closure for unsafe call
        let target_display_id = display_id.unwrap_or_else(|| CGMainDisplayID() );
        let bounds = CGDisplayBounds(target_display_id);
        if bounds.size.width == 0.0 || bounds.size.height == 0.0 {
            Err(AutomationError::PlatformError(format!("Invalid display bounds for display ID: {:?}", target_display_id)))
        } else {
            Ok(bounds)
        }
    }
}

/// Convert global screen coordinates to window-relative coordinates.
/// Returns (x, y) relative to the window's top-left corner.
pub fn global_to_window_coordinates(
    global_x: f64,
    global_y: f64,
    window_element: &MacOSUIElement,
) -> Result<(f64, f64), AutomationError> {
    // Verify that the element is a window
    let attrs = window_element.attributes();
    if attrs.role != "AXWindow" {
        return Err(AutomationError::PlatformError(format!(
            "Element is not a window. Expected role 'AXWindow', got '{}'",
            attrs.role
        )));
    }

    // Get window bounds
    let (window_x, window_y, _width, _height) = window_element.bounds()?;

    // Convert to window-relative coordinates
    let relative_x = global_x - window_x;
    let relative_y = global_y - window_y;

    Ok((relative_x, relative_y))
}

/// Convert window-relative coordinates to global screen coordinates.
/// Takes (x, y) relative to the window's top-left corner and returns global coordinates.
pub fn window_to_global_coordinates(
    window_x: f64,
    window_y: f64,
    window_element: &MacOSUIElement,
) -> Result<(f64, f64), AutomationError> {
    // Verify that the element is a window
    let attrs = window_element.attributes();
    if attrs.role != "AXWindow" {
        return Err(AutomationError::PlatformError(format!(
            "Element is not a window. Expected role 'AXWindow', got '{}'",
            attrs.role
        )));
    }

    // Get window bounds
    let (global_window_x, global_window_y, _width, _height) = window_element.bounds()?;

    // Convert to global coordinates
    let global_x = global_window_x + window_x;
    let global_y = global_window_y + window_y;

    Ok((global_x, global_y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macos_role_to_generic_role_known() {
        assert_eq!(macos_role_to_generic_role("AXWindow"), vec!["window"]);
        assert_eq!(macos_role_to_generic_role("AXButton"), vec!["button"]);
        assert_eq!(macos_role_to_generic_role("AXMenuItem"), vec!["button"]);
        assert_eq!(macos_role_to_generic_role("AXMenuBarItem"), vec!["button"]);
        assert_eq!(macos_role_to_generic_role("axtextfield"), vec!["textfield", "input", "textbox", "url", "urlfield"]); // Case insensitive
        assert_eq!(macos_role_to_generic_role("AXList"), vec!["list"]);
        assert_eq!(macos_role_to_generic_role("AXCell"), vec!["listitem"]);
        assert_eq!(macos_role_to_generic_role("AXSheet"), vec!["dialog"]);
        assert_eq!(macos_role_to_generic_role("AXDialog"), vec!["dialog"]);
        assert_eq!(macos_role_to_generic_role("AXGroup"), vec!["group", "genericElement"]);
    }

    #[test]
    fn test_macos_role_to_generic_role_unknown() {
        assert_eq!(macos_role_to_generic_role("AXUnknownRole"), vec!["AXUnknownRole"]);
        assert_eq!(macos_role_to_generic_role("SomeOtherRole"), vec!["SomeOtherRole"]);
    }

    #[test]
    fn test_macos_role_to_generic_role_case_insensitivity() {
        assert_eq!(macos_role_to_generic_role("axwindow"), vec!["window"]);
        assert_eq!(macos_role_to_generic_role("aXbUtToN"), vec!["button"]);
    }

    #[test]
    fn test_macos_role_to_generic_role_textfield_variants() {
        let expected = vec!["textfield", "input", "textbox", "url", "urlfield"];
        assert_eq!(macos_role_to_generic_role("AXTextField"), expected);
        assert_eq!(macos_role_to_generic_role("AXTextArea"), expected);
        assert_eq!(macos_role_to_generic_role("AXTextEdit"), expected);
        assert_eq!(macos_role_to_generic_role("AXSearchField"), expected);
        assert_eq!(macos_role_to_generic_role("AXURIField"), expected);
        assert_eq!(macos_role_to_generic_role("AXAddressField"), expected);
    }

    #[test]
    fn test_macos_role_to_generic_role_group_variants() {
        let expected = vec!["group", "genericElement"];
        assert_eq!(macos_role_to_generic_role("AXGroup"), expected);
        assert_eq!(macos_role_to_generic_role("AXGenericElement"), expected);
        assert_eq!(macos_role_to_generic_role("AXWebArea"), expected);
    }
}
