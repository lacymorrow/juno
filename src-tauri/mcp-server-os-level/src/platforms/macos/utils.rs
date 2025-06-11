use accessibility::{AXAttribute, AXUIElement, AXUIElementAttributes}; // Import AXUIElementAttributes trait
use base64::prelude::{Engine as _, BASE64_STANDARD};
use core_foundation::string::CFString;
use image::{ImageBuffer, ImageFormat, Rgba, imageops}; // Use ImageFormat instead of ImageOutputFormat. Removed unused Pixel, RgbaImage. Added imageops
use std::io::Cursor; // Added for image encoding
use tracing::{debug, warn}; // Added warn
use super::element::MacOSUIElement; // Added for the new function
use crate::element::UIElementImpl; // Import the trait providing .attributes()
use crate::AutomationError; // From Main

// Import Cidre for safe Apple framework access (conditional)
#[cfg(all(target_os = "macos", feature = "use-cidre"))]
use cidre::{ns, cg, cf};
#[cfg(all(target_os = "macos", feature = "use-cidre"))]
use cidre::ns::ApplicationActivationPolicy;

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

// Safe Cidre implementation for getting PIDs of running applications
#[cfg(all(target_os = "macos", feature = "use-cidre"))]
pub(crate) fn get_running_application_pids(
    use_background_apps: bool,
) -> Result<Vec<i32>, AutomationError> {
    debug!("Getting running application PIDs using safe Cidre implementation");

    // Use Cidre's safe NSWorkspace API
    let workspace = ns::Workspace::shared();
    let apps = workspace.running_applications();

    let mut pids = Vec::new();
    
    for app in apps.iter() {
        // Filter apps by activation policy if requested
        if !use_background_apps {
            let activation_policy = app.activation_policy();
            match activation_policy {
                ApplicationActivationPolicy::Prohibited | ApplicationActivationPolicy::Accessory => {
                    continue; // Skip background/accessory apps
                }
                ApplicationActivationPolicy::Regular => {
                    // Include regular apps
                }
            }
        }

        // Filter out common background workers by bundle identifier
        if let Some(bundle_id) = app.bundle_identifier() {
            let bundle_id_str = bundle_id.to_string();
            
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

        let pid = app.process_identifier();
        pids.push(pid);
    }

    debug!("Found {} application PIDs using Cidre", pids.len());
    Ok(pids)
}

// Fallback implementation using Objective-C when Cidre is not available
#[cfg(all(target_os = "macos", not(feature = "use-cidre")))]
pub(crate) fn get_running_application_pids(
    use_background_apps: bool,
) -> Result<Vec<i32>, AutomationError> {
    debug!("Getting running application PIDs using Objective-C fallback");

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

        debug!("Found {} application PIDs using Objective-C", pids.len());
        Ok(pids)
    }
}

// Fallback implementation for non-macOS targets
#[cfg(not(target_os = "macos"))]
pub(crate) fn get_running_application_pids(
    _use_background_apps: bool,
) -> Result<Vec<i32>, AutomationError> {
    Err(AutomationError::PlatformError(
        "NSWorkspace functionality is only available on macOS".to_string()
    ))
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

/// Captures a screenshot of the main display and encodes it as base64 PNG using Cidre
pub fn capture_and_encode_screenshot() -> Result<String, AutomationError> {
    // 1. Get current cursor position using Cidre
    let cursor_point = get_cursor_position_cidre()?;
    debug!("Current cursor position: ({}, {})", cursor_point.0, cursor_point.1);

    // 2. Find the display containing the cursor using Cidre
    let target_display_id = match find_display_containing_point_cidre(cursor_point.0, cursor_point.1) {
        Ok(id) => {
            debug!("Cursor found on display ID: {}", id);
            id
        },
        Err(e) => {
            warn!("Failed to find display for cursor at ({}, {}): {}. Falling back to main display.", cursor_point.0, cursor_point.1, e);
            get_main_display_id_cidre()?
        }
    };

    // 3. Capture the specific display containing the cursor using Cidre
    let cg_image = capture_screenshot_cgimage_cidre(Some(target_display_id))?;
    debug!("Captured screenshot for display ID: {}", target_display_id);

    // 4. Convert CGImage to buffer first
    let buffer = cgimage_to_imagebuffer_cidre(cg_image)?;
    // 5. Encode
    encode_imagebuffer_to_base64_png(&buffer)
}

/// Safe Cidre implementation for getting cursor position
#[cfg(target_os = "macos")]
fn get_cursor_position_cidre() -> Result<(f64, f64), AutomationError> {
    let event_source = cg::EventSource::new(cg::EventSourceStateId::HidSystemState)
        .map_err(|e| AutomationError::PlatformError(format!("Failed to create HID event source: {:?}", e)))?;
    let event = cg::Event::new(&event_source)
        .ok_or_else(|| AutomationError::PlatformError("Failed to create null CGEvent to get location".to_string()))?;
    let location = event.location();
    Ok((location.x, location.y))
}

#[cfg(not(target_os = "macos"))]
fn get_cursor_position_cidre() -> Result<(f64, f64), AutomationError> {
    Err(AutomationError::PlatformError("Cursor position only available on macOS".to_string()))
}

/// Safe Cidre implementation for getting main display ID
#[cfg(target_os = "macos")]
fn get_main_display_id_cidre() -> Result<u32, AutomationError> {
    Ok(cg::Display::main().id())
}

#[cfg(not(target_os = "macos"))]
fn get_main_display_id_cidre() -> Result<u32, AutomationError> {
    Err(AutomationError::PlatformError("Display functionality only available on macOS".to_string()))
}

/// Safe Cidre implementation for finding display containing point
#[cfg(target_os = "macos")]
fn find_display_containing_point_cidre(x: f64, y: f64) -> Result<u32, AutomationError> {
    debug!("Finding display containing point ({}, {}) using Cidre", x, y);

    let point = cg::Point::new(x, y);
    let displays = cg::Display::active_displays()
        .map_err(|e| AutomationError::PlatformError(format!("Failed to get active displays: {:?}", e)))?;

    for display in displays.iter() {
        let bounds = display.bounds();
        let rect = cg::Rect::new(bounds.origin, bounds.size);
        
        if rect.contains(&point) {
            let display_id = display.id();
            debug!("Point ({}, {}) is on display {}", x, y, display_id);
            return Ok(display_id);
        }
    }

    // Fallback to main display
    let main_display = cg::Display::main();
    let display_id = main_display.id();
    debug!("Point ({}, {}) not found on any display, defaulting to main display {}", x, y, display_id);
    Ok(display_id)
}

#[cfg(not(target_os = "macos"))]
fn find_display_containing_point_cidre(_x: f64, _y: f64) -> Result<u32, AutomationError> {
    Err(AutomationError::PlatformError("Display functionality only available on macOS".to_string()))
}

/// Captures a screenshot of a specific UI element and encodes it as base64 PNG using Cidre
pub fn capture_element_screenshot(element: &MacOSUIElement) -> Result<String, AutomationError> {
    let (x, y, width, height) = element.bounds()?;

    // Add check for zero or negative dimensions immediately after getting bounds
    if width <= 0.0 || height <= 0.0 {
        let attrs = element.attributes();
        let role = attrs.role;
        let label = attrs.label.unwrap_or_else(|| "N/A".to_string());
        warn!(
            "Cannot capture screenshot for element with zero or negative dimensions. Role: '{}', Label: '{}', Bounds: ({}, {}, {}, {})",
            role, label, x, y, width, height
        );
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

    // Find the display containing the element's center point using Cidre
    let target_display_id = match find_display_containing_point_cidre(center_x, center_y) {
        Ok(id) => id,
        Err(_) => {
            warn!(
                "Could not determine display for element at ({}, {}). Defaulting to main display.",
                center_x, center_y
            );
            get_main_display_id_cidre()?
        }
    };

    // Get the bounds of the target display using Cidre
    let display_bounds = get_display_bounds_cidre(Some(target_display_id))?;

    // Adjust element coordinates to be relative to the target display's origin
    let relative_x = x - display_bounds.0;
    let relative_y = y - display_bounds.1;

    // Create a CGRect for the element's bounds, now relative to the target display
    let crop_x = relative_x.max(0.0).floor() as u32;
    let crop_y = relative_y.max(0.0).floor() as u32;
    let crop_width = width.max(1.0).ceil() as u32;
    let crop_height = height.max(1.0).ceil() as u32;

    // Capture the target display using Cidre
    let display_cg_image = capture_screenshot_cgimage_cidre(Some(target_display_id))?;

    // Convert the target display's CGImage to an ImageBuffer
    let display_buffer = cgimage_to_imagebuffer_cidre(display_cg_image)?;

    // Crop the ImageBuffer
    if crop_x + crop_width > display_buffer.width() || crop_y + crop_height > display_buffer.height() {
        warn!(
            "Element bounds ({}, {}, {}, {}) exceed screen dimensions ({}, {}). Clamping crop.",
            crop_x, crop_y, crop_width, crop_height, display_buffer.width(), display_buffer.height()
        );
        
        let clamped_width = crop_width.min(display_buffer.width().saturating_sub(crop_x));
        let clamped_height = crop_height.min(display_buffer.height().saturating_sub(crop_y));
        
        if clamped_width == 0 || clamped_height == 0 {
            let attrs = element.attributes();
            let role = attrs.role;
            let label = attrs.label.unwrap_or_else(|| "N/A".to_string());
            let err_msg = format!(
                "Element bounds result in zero-size crop area after clamping. Role: '{}', Label: '{}', Original Bounds: ({}, {}, {}, {}), Clamped Crop: ({}, {}, {}, {})",
                role, label, x, y, width, height, crop_x, crop_y, clamped_width, clamped_height
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
        let attrs = element.attributes();
        let role = attrs.role;
        let label = attrs.label.unwrap_or_else(|| "N/A".to_string());
        let err_msg = format!(
            "Element bounds result in zero-size crop area before clamping (likely due to u32 conversion). Role: '{}', Label: '{}', Original Bounds: ({}, {}, {}, {})",
            role, label, x, y, width, height
        );
        warn!("{}", err_msg);
        return Err(AutomationError::PlatformError(err_msg));
    } else {
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

/// Captures a screenshot of a specific window and encodes it as base64 PNG using Cidre
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

    // Find the display containing the window's center point using Cidre
    let target_display_id = match find_display_containing_point_cidre(center_x, center_y) {
        Ok(id) => id,
        Err(_) => {
            warn!(
                "Could not determine display for window at ({}, {}). Defaulting to main display.",
                center_x, center_y
            );
            get_main_display_id_cidre()?
        }
    };

    // Get the bounds of the target display using Cidre
    let display_bounds = get_display_bounds_cidre(Some(target_display_id))?;

    // Adjust window coordinates to be relative to the target display's origin
    let relative_x = x - display_bounds.0;
    let relative_y = y - display_bounds.1;

    // Create crop parameters, ensuring they're positive and within display bounds
    let crop_x = relative_x.max(0.0).floor() as u32;
    let crop_y = relative_y.max(0.0).floor() as u32;
    let crop_width = width.max(1.0).ceil() as u32;
    let crop_height = height.max(1.0).ceil() as u32;

    // Capture the target display using Cidre
    let display_cg_image = capture_screenshot_cgimage_cidre(Some(target_display_id))?;

    // Convert to ImageBuffer
    let display_buffer = cgimage_to_imagebuffer_cidre(display_cg_image)?;

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
        return Err(AutomationError::PlatformError(err_msg));
    } else {
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

/// Safe Cidre implementation for converting CGImage to ImageBuffer
#[cfg(target_os = "macos")]
fn cgimage_to_imagebuffer_cidre(cg_image: cg::Image) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, AutomationError> {
    let width = cg_image.width();
    let height = cg_image.height();
    let data = cg_image.data_provider()
        .and_then(|provider| provider.copy_data())
        .ok_or_else(|| AutomationError::PlatformError("Failed to get image data".to_string()))?;
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
    let bytes_per_row = cg_image.bytes_per_row();
    
    for y in 0..height {
        for x in 0..width {
            let index = (y * bytes_per_row) + (x * 4);
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

#[cfg(not(target_os = "macos"))]
fn cgimage_to_imagebuffer_cidre(_cg_image: ()) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, AutomationError> {
    Err(AutomationError::PlatformError("CGImage functionality only available on macOS".to_string()))
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

/// Safe Cidre implementation for capturing screenshot
#[cfg(target_os = "macos")]
fn capture_screenshot_cgimage_cidre(display_id: Option<u32>) -> Result<cg::Image, AutomationError> {
    let target_display_id = display_id.unwrap_or_else(|| {
        warn!("capture_screenshot_cgimage_cidre called with None display_id, defaulting to main display.");
        cg::Display::main().id()
    });
    
    let display = cg::Display::from_id(target_display_id);
    let bounds = display.bounds();
    
    display.create_image(&bounds)
        .ok_or_else(|| {
            AutomationError::PlatformError(format!(
                "Failed to capture screenshot for display ID {}", 
                target_display_id
            ))
        })
}

#[cfg(not(target_os = "macos"))]
fn capture_screenshot_cgimage_cidre(_display_id: Option<u32>) -> Result<(), AutomationError> {
    Err(AutomationError::PlatformError("Screenshot functionality only available on macOS".to_string()))
}

/// Safe Cidre implementation for getting display bounds
#[cfg(target_os = "macos")]
pub fn get_display_bounds_cidre(display_id: Option<u32>) -> Result<(f64, f64, f64, f64), AutomationError> {
    let target_display_id = display_id.unwrap_or_else(|| cg::Display::main().id());
    let display = cg::Display::from_id(target_display_id);
    let bounds = display.bounds();
    
    Ok((bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height))
}

#[cfg(not(target_os = "macos"))]
pub fn get_display_bounds_cidre(_display_id: Option<u32>) -> Result<(f64, f64, f64, f64), AutomationError> {
    Err(AutomationError::PlatformError("Display functionality only available on macOS".to_string()))
}

/// Checks if the current process has accessibility permissions using safe Cidre implementation
pub fn check_accessibility_permissions() -> bool {
    #[cfg(target_os = "macos")]
    {
        use crate::platforms::macos::ffi::ax_is_process_trusted_with_options;
        ax_is_process_trusted_with_options(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Safe wrapper around get_display_bounds_cidre that returns CGRect-like structure
pub fn get_display_bounds(display_id: Option<u32>) -> Result<(f64, f64, f64, f64), AutomationError> {
    get_display_bounds_cidre(display_id)
}

pub fn global_to_window_coordinates(
    global_x: f64,
    global_y: f64,
    window_element: &MacOSUIElement,
) -> Result<(f64, f64), AutomationError> {
    let (window_x, window_y, _, _) = window_element.bounds()?;
    
    let local_x = global_x - window_x;
    let local_y = global_y - window_y;
    
    Ok((local_x, local_y))
}

pub fn window_to_global_coordinates(
    window_x: f64,
    window_y: f64,
    window_element: &MacOSUIElement,
) -> Result<(f64, f64), AutomationError> {
    let (element_x, element_y, _, _) = window_element.bounds()?;
    
    let global_x = window_x + element_x;
    let global_y = window_y + element_y;
    
    Ok((global_x, global_y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macos_role_to_generic_role_known() {
        let result = macos_role_to_generic_role("AXWindow");
        assert_eq!(result, vec!["window"]);

        let result = macos_role_to_generic_role("AXButton");
        assert_eq!(result, vec!["button"]);

        let result = macos_role_to_generic_role("AXTextField");
        assert!(result.contains(&"textfield".to_string()));
        assert!(result.contains(&"input".to_string()));
    }

    #[test]
    fn test_macos_role_to_generic_role_unknown() {
        let result = macos_role_to_generic_role("AXUnknownRole");
        assert_eq!(result, vec!["AXUnknownRole"]);
    }

    #[test]
    fn test_macos_role_to_generic_role_case_insensitivity() {
        let result1 = macos_role_to_generic_role("AXWindow");
        let result2 = macos_role_to_generic_role("axwindow");
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_macos_role_to_generic_role_textfield_variants() {
        let variants = ["AXTextField", "AXTextArea", "AXTextEdit", "AXSearchField"];
        for variant in &variants {
            let result = macos_role_to_generic_role(variant);
            assert!(result.contains(&"textfield".to_string()));
            assert!(result.contains(&"input".to_string()));
        }
    }

    #[test]
    fn test_macos_role_to_generic_role_group_variants() {
        let variants = ["AXGroup", "AXGenericElement", "AXWebArea"];
        for variant in &variants {
            let result = macos_role_to_generic_role(variant);
            assert!(result.contains(&"group".to_string()));
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_get_running_application_pids_cidre() {
        let result = get_running_application_pids(false);
        assert!(result.is_ok());
        let pids = result.unwrap();
        assert!(!pids.is_empty(), "Should find at least some running applications");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_get_display_bounds_cidre() {
        let result = get_display_bounds_cidre(None);
        assert!(result.is_ok());
        let (x, y, width, height) = result.unwrap();
        assert!(width > 0.0 && height > 0.0, "Display should have positive dimensions");
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_functions_fail_on_non_macos() {
        assert!(get_running_application_pids(false).is_err());
        assert!(get_display_bounds_cidre(None).is_err());
    }
}
