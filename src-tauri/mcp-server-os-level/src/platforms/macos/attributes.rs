use super::constants::{K_AXVALUE_CGPOINT_TYPE, K_AXVALUE_CGSIZE_TYPE};
use super::element::MacOSUIElement;
use super::ffi::AXValueGetValue;
use crate::UIElementAttributes;
use accessibility::AXUIElement;
use core_foundation::array::{
    CFArrayGetCount, CFArrayGetTypeID, CFArrayGetValueAtIndex, __CFArray,
};
use core_foundation::base::{CFGetTypeID, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::geometry::{CGPoint, CGSize};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::debug;

// Helper function to parse AXUIElement attribute values into appropriate types
// Moved from utils.rs
pub(crate) fn parse_ax_attribute_value(
    name: &str,
    value: core_foundation::base::CFType,
) -> Option<serde_json::Value> {
    // --- Start Added Logging ---
    let value_type_id = value.type_of();
    debug!(
        "parse_ax_attribute_value: Processing attribute '{}', CFTypeID: {}",
        name, value_type_id
    );
    // --- End Added Logging ---

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
                        // --- Start Added Logging ---
                        debug!(
                            "parse_ax_attribute_value: Numeric value for '{}' is non-finite (NaN/Infinity). Returning Null.",
                            name
                        );
                        // --- End Added Logging ---
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
                } else {
                    // --- Start Added Logging (moved inside else branch) ---
                    debug!(
                        "parse_ax_attribute_value: Failed to parse AXChildren attribute '{}'. Expected CFArray, got TypeID {}. Returning None.",
                        name, type_id
                    );
                    // --- End Added Logging ---
                    return None; // Return None directly from here
                }
            }
        }

        _ => {}
    }

    // Fallback for unhandled types
    // --- Start Added Logging ---
    debug!(
        "parse_ax_attribute_value: Attribute '{}' with CFTypeID {} was not handled by specific cases or failed downcasting. Returning None.",
        name, value_type_id
    );
    // --- End Added Logging ---
    None
}

// Moved from MacOSUIElement implementation in element.rs
pub(crate) fn get_element_attributes(_element: &MacOSUIElement) -> UIElementAttributes {
    // This function is no longer used directly to populate MacOSUIElement.attributes()
    // It's kept temporarily for potential future use or reference, but its body
    // has been effectively moved/merged into the fallback logic within
    // MacOSUIElement::attributes() in element.rs.
    // We return a default/empty struct to satisfy the signature.
    let properties = HashMap::new();
    UIElementAttributes {
        role: String::new(),
        label: None,
        value: None,
        description: None,
        properties,
    }
    // Original implementation commented out below for reference
    /*
    let mut properties = HashMap::new();
    let is_window = element
        .element
        .0
        .role() // Keep this initial check for window determination, might be okay
        .map_or(false, |r| r.to_string() == "AXWindow");

    // --- Start Refactor: Use attribute() consistently ---
    let mut raw_role = String::new();
    let role_attr = AXAttribute::new(&CFString::new("AXRole"));
    if let Ok(value) = element.element.0.attribute(&role_attr) {
        if let Some(cf_string) = value.downcast_into::<CFString>() {
            raw_role = cf_string.to_string();
        }
    }
    let generic_role = macos_role_to_generic_role(&raw_role)
        .first()
        .unwrap_or(&raw_role)
        .to_string();

    debug!(
        "get_element_attributes: Fetched raw role via attribute(): '{}', determined generic role '{}'",
        raw_role, generic_role
    );
    // --- End Refactor ---

    let mut attrs = UIElementAttributes {
        role: if is_window {
            "window".to_string()
        } else {
            generic_role.clone()
        },
        label: None,
        value: None,
        description: None,
        properties,
    };

    if is_window {
        // Window-specific logic remains largely the same, but uses attribute()
        let title_attrs = [
            "AXTitle",
            "AXTitleUIElement",
            "AXDocument",
            "AXFilename",
            "AXName",
        ];
        for title_attr_name in title_attrs {
            let title_attr = AXAttribute::new(&CFString::new(title_attr_name));
            if let Ok(value) = element.element.0.attribute(&title_attr) {
                if let Some(cf_string) = value.downcast_into::<CFString>() {
                    attrs.label = Some(cf_string.to_string());
                    debug!(
                        "Window label set from {}: {:?}",
                        title_attr_name, attrs.label
                    );
                    break;
                }
            }
        }
        let std_attrs = ["AXMinimized", "AXMain", "AXFocused"];
        for attr_name in std_attrs {
            let attr = AXAttribute::new(&CFString::new(attr_name));
            if let Ok(value) = element.element.0.attribute(&attr) {
                // Use parse_ax_attribute_value for consistency
                let parsed_value = parse_ax_attribute_value(attr_name, value);
                attrs.properties.insert(attr_name.to_string(), parsed_value);
                debug!(
                    "Window property {} set: {:?}",
                    attr_name,
                    attrs.properties.get(attr_name)
                );
            } else {
                debug!("Window property {} not found or error.", attr_name);
            }
        }
    } else {
        // --- Start Refactor: Use attribute() for standard fields ---
        debug!("Fetching attributes for non-window element using attribute().");

        // Label
        let title_attr = AXAttribute::new(&CFString::new("AXTitle"));
        if let Ok(value) = element.element.0.attribute(&title_attr) {
            if let Some(cf_string) = value.downcast_into::<CFString>() {
                let title_str = cf_string.to_string();
                if !title_str.is_empty() {
                    attrs.label = Some(title_str);
                }
            }
        }
        debug!("Attempted label via AXTitle: {:?}", attrs.label);
        // Try AXLabel if AXTitle didn't work or was empty
        if attrs.label.is_none() {
            let label_attr = AXAttribute::new(&CFString::new("AXLabel"));
            if let Ok(value) = element.element.0.attribute(&label_attr) {
                if let Some(cf_string) = value.downcast_into::<CFString>() {
                    attrs.label = Some(cf_string.to_string());
                }
            }
            debug!("Attempted label via AXLabel: {:?}", attrs.label);
        }

        // Description
        let desc_attr = AXAttribute::new(&CFString::new("AXDescription"));
        if let Ok(value) = element.element.0.attribute(&desc_attr) {
            if let Some(cf_string) = value.downcast_into::<CFString>() {
                attrs.description = Some(cf_string.to_string());
            }
        }
        debug!(
            "Attempted description via AXDescription: {:?}",
            attrs.description
        );

        // Value
        let value_attr = AXAttribute::new(&CFString::new("AXValue"));
        match element.element.0.attribute(&value_attr) {
            Ok(value) => {
                // Use the existing parse logic, but handle the direct result
                if let Some(cf_string) = value.clone().downcast_into::<CFString>() {
                    attrs.value = Some(cf_string.to_string());
                    debug!("Got value as CFString via attribute(): {:?}", attrs.value);
                } else if let Some(cf_num) = value.clone().downcast_into::<CFNumber>() {
                    if let Some(num) = cf_num.to_i64() {
                        attrs.value = Some(num.to_string());
                        debug!(
                            "Got value as CFNumber (i64) via attribute(): {:?}",
                            attrs.value
                        );
                    } else if let Some(num) = cf_num.to_f64() {
                        attrs.value = Some(num.to_string());
                        debug!(
                            "Got value as CFNumber (f64) via attribute(): {:?}",
                            attrs.value
                        );
                    } else {
                        debug!("Got value as CFNumber via attribute(), but couldn't convert to i64 or f64.");
                    }
                } else {
                    let type_id = value.type_of();
                    debug!(
                        "Got AXValue attribute via attribute(), but it's not CFString or CFNumber. TypeID: {}",
                        type_id
                    );
                }
            }
            Err(e) => {
                debug!(
                    "Failed to get AXValue attribute via attribute(). Error: {:?}",
                    e
                );
            }
        }
        // --- End Refactor ---
    }

    // Fetching other properties remains the same
    if let Ok(attr_names) = element.element.0.attribute_names() {
        for name in attr_names.iter() {
            let attr = AXAttribute::new(&name);
            let name_str = name.to_string();
            if ![
                "AXRole",        // Already handled
                "AXTitle",       // Already handled (or part of label logic)
                "AXLabel",       // Already handled (part of label logic)
                "AXDescription", // Already handled
                "AXValue",       // Already handled
                "AXMinimized",   // Already handled for windows
                "AXMain",        // Already handled for windows
                "AXFocused",     // Already handled for windows
                "AXPosition",    // Handled by bounds() method
                "AXSize",        // Handled by bounds() method
            ]
            .contains(&name_str.as_str())
            {
                match element.element.0.attribute(&attr) {
                    Ok(value) => {
                        let parsed_value = parse_ax_attribute_value(&name_str, value);
                        attrs.properties.insert(name_str, parsed_value);
                    }
                    Err(e) => {
                        // --- Start Modified Logging ---
                        // Log errors unless they are common "expected" failures
                        if !matches!(
                            e,
                            accessibility::Error::Ax(-25212) // attribute unsupported
                                | accessibility::Error::Ax(-25205) // no value
                                | accessibility::Error::Ax(-25204) // getting attribute failed (internal error)
                        ) {
                            debug!("Error getting property attribute '{}': {:?}", name_str, e);
                        } else {
                            // Optionally log ignored errors at a lower level if needed
                            // trace!("Ignoring expected error for property attribute '{}': {:?}", name_str, e);
                        }
                        // --- End Modified Logging ---
                    }
                }
            }
        }
    }
    attrs
    */
}
