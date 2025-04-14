use super::wrappers::ThreadSafeAXUIElement;
use crate::AutomationError;
use crate::platforms::macos::ffi::AXValueGetValue; // Import from ffi
use crate::platforms::macos::constants::{K_AXVALUE_CGPOINT_TYPE, K_AXVALUE_CGSIZE_TYPE}; // Import from constants
use accessibility::{AXAttribute, AXUIElement, AXUIElementAttributes}; // Import AXUIElementAttributes trait
use core_foundation::array::{CFArrayGetCount, CFArrayGetTypeID, CFArrayGetValueAtIndex, __CFArray};
use core_foundation::base::{CFGetTypeID, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::CFString;
use core_graphics::geometry::{CGPoint, CGSize};
use serde_json::{json, Value};
use tracing::debug;


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
pub(crate) fn get_running_application_pids(use_background_apps: bool) -> Result<Vec<i32>, AutomationError> {
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

// Helper function to parse AXUIElement attribute values into appropriate types
pub(crate) fn parse_ax_attribute_value(
    name: &str,
    value: core_foundation::base::CFType,
) -> Option<serde_json::Value> {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::geometry::{CGPoint, CGSize};
    use serde_json::{json, Value};

    // Handle different types based on known attribute names and value types
    match name {
        // String values (text, identifiers, descriptions)
        "AXRole" | "AXRoleDescription" | "AXIdentifier" | "AXValue" => {
            if let Some(cf_string) = value.downcast_into::<CFString>() {
                return Some(Value::String(cf_string.to_string()));
            }
        }

        // Boolean values
        "AXEnabled" | "AXFocused" => {
            if let Some(cf_bool) = value.downcast_into::<CFBoolean>() {
                return Some(Value::Bool(cf_bool == CFBoolean::true_value()));
            }
        }

        // Numeric values
        "AXNumberOfCharacters" | "AXInsertionPointLineNumber" => {
            if let Some(cf_num) = value.downcast_into::<CFNumber>() {
                if let Some(num) = cf_num.to_i64() {
                    return Some(Value::Number(serde_json::Number::from(num)));
                } else if let Some(num) = cf_num.to_f64() {
                    // Need to handle possible NaN/Infinity which aren't allowed in JSON
                    if num.is_finite() {
                        return serde_json::Number::from_f64(num).map(Value::Number);
                    } else {
                        return Some(Value::Null);
                    }
                }
            }
        }

        // Position, Size and Frame require special handling with AXValue
        "AXPosition" => {
            // Try to extract CGPoint using AXValueGetValue
            unsafe {
                let value_ref = value.as_CFTypeRef();
                let mut point = CGPoint { x: 0.0, y: 0.0 };
                let point_ptr = &mut point as *mut CGPoint as *mut ::std::os::raw::c_void;

                if AXValueGetValue(value_ref, K_AXVALUE_CGPOINT_TYPE, point_ptr) != 0 {
                    return Some(json!({
                        "x": point.x,
                        "y": point.y
                    }));
                }
            }
        }

        "AXSize" => {
            // Try to extract CGSize using AXValueGetValue
            unsafe {
                let value_ref = value.as_CFTypeRef();
                let mut size = CGSize {
                    width: 0.0,
                    height: 0.0,
                };
                let size_ptr = &mut size as *mut CGSize as *mut ::std::os::raw::c_void;

                if AXValueGetValue(value_ref, K_AXVALUE_CGSIZE_TYPE, size_ptr) != 0 {
                    return Some(json!({
                        "width": size.width,
                        "height": size.height
                    }));
                }
            }
        }

        // For attributes that are references to other UI elements
        "AXParent" | "AXWindow" | "AXTopLevelUIElement" => {
            // get object id
            if let Some(ax_element) = value.downcast_into::<AXUIElement>() {
                let address = &ax_element as *const _ as usize;
                return Some(Value::String(format!("{}", address)));
            }
        }

        // For array types (children)
        name if name.starts_with("AXChildren") => {
            // debug!("Processing AXChildren attribute");

            unsafe {
                let value_ref = value.as_CFTypeRef();
                let type_id = CFGetTypeID(value_ref);

                if type_id == CFArrayGetTypeID() {
                    // Cast to CFArrayRef
                    let array_ref = value_ref as *const __CFArray;
                    let count = CFArrayGetCount(array_ref);
                    // debug!("AXChildren array with {} elements", count);

                    // Create an array of element addresses
                    let mut items = Vec::with_capacity(count as usize);
                    for i in 0..count {
                        let item = CFArrayGetValueAtIndex(array_ref, i as isize);
                        if !item.is_null() {
                            // Correctly wrap the raw pointer into AXUIElement
                            let ax_element = AXUIElement::wrap_under_get_rule(item as *mut _);
                            let address = &ax_element as *const _ as usize;
                            items.push(json!(format!("{}", address)));
                        }
                    }
                    return Some(Value::Array(items));
                }
            }

            return None;
        }

        _ => {}
    }

    // Fallback for unhandled types
    None
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
    for attr_name in &[ // Changed to &[&str]
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
