use super::constants::{K_AXVALUE_CGPOINT_TYPE, K_AXVALUE_CGSIZE_TYPE};
use super::element::MacOSUIElement;
use super::ffi::AXValueGetValue;
use super::utils::macos_role_to_generic_role;
use crate::UIElementAttributes;
use accessibility::{AXAttribute, AXUIElement, AXUIElementAttributes as AXAttrsTrait};
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

// Moved from MacOSUIElement implementation in element.rs
pub(crate) fn get_element_attributes(element: &MacOSUIElement) -> UIElementAttributes {
    let properties = HashMap::new();
    let is_window = element
        .element
        .0
        .role()
        .map_or(false, |r| r.to_string() == "AXWindow");

    // Determine role based on the raw AXRole first
    let raw_role = element
        .element
        .0
        .role()
        .map(|r| r.to_string())
        .unwrap_or_default();
    let generic_role = macos_role_to_generic_role(&raw_role)
        .first()
        .unwrap_or(&raw_role)
        .to_string();

    let mut attrs = UIElementAttributes {
        role: generic_role.clone(), // Use the determined generic role
        label: None,
        value: None,
        description: None,
        properties,
    };

    if is_window {
        attrs.role = "window".to_string(); // Explicitly set role for windows
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
                    break;
                }
            }
        }
        let std_attrs = ["AXMinimized", "AXMain", "AXFocused"];
        for attr_name in std_attrs {
            let attr = AXAttribute::new(&CFString::new(attr_name));
            if let Ok(value) = element.element.0.attribute(&attr) {
                if let Some(cf_bool) = value.downcast_into::<CFBoolean>() {
                    attrs.properties.insert(
                        attr_name.to_string(),
                        Some(serde_json::Value::String(format!("{:?}", cf_bool))),
                    );
                }
            }
        }
    } else {
        // --- Start Added Logging ---
        debug!("Fetching attributes for non-window element.");
        // --- End Added Logging ---
        attrs.label = element.element.0.title().ok().map(|s| s.to_string());
        // --- Start Added Logging ---
        debug!("Attempted to get label via title(): {:?}", attrs.label);
        // --- End Added Logging ---
        if attrs.label.is_none() {
            let label_attr = AXAttribute::new(&CFString::new("AXLabel"));
            let label_result = element.element.0.attribute(&label_attr);
            // --- Start Added Logging ---
            debug!(
                "Attempted to get label via AXLabel attribute: {:?}",
                label_result
            );
            // --- End Added Logging ---
            attrs.label = label_result
                .ok()
                .and_then(|val| val.downcast_into::<CFString>())
                .map(|s| s.to_string());
            // --- Start Added Logging ---
            debug!("Final label after AXLabel check: {:?}", attrs.label);
            // --- End Added Logging ---
        }
        let description_result = element.element.0.description();
        // --- Start Added Logging ---
        debug!("Attempted to get description(): {:?}", description_result);
        // --- End Added Logging ---
        attrs.description = description_result.ok().map(|s| s.to_string());

        let value_attr = AXAttribute::new(&CFString::new("AXValue"));
        let value_result = element.element.0.attribute(&value_attr);
        // --- Start Added Logging ---
        debug!(
            "Attempted to get value via AXValue attribute: {:?}",
            value_result
        );
        // --- End Added Logging ---
        if let Ok(value) = value_result {
            if let Some(cf_string) = value.clone().downcast_into::<CFString>() {
                attrs.value = Some(cf_string.to_string());
                // --- Start Added Logging ---
                debug!("Got value as CFString: {:?}", attrs.value);
                // --- End Added Logging ---
            } else if let Some(cf_num) = value.clone().downcast_into::<CFNumber>() {
                if let Some(num) = cf_num.to_i64() {
                    attrs.value = Some(num.to_string());
                    // --- Start Added Logging ---
                    debug!("Got value as CFNumber (i64): {:?}", attrs.value);
                    // --- End Added Logging ---
                } else if let Some(num) = cf_num.to_f64() {
                    attrs.value = Some(num.to_string());
                    // --- Start Added Logging ---
                    debug!("Got value as CFNumber (f64): {:?}", attrs.value);
                    // --- End Added Logging ---
                } else {
                    // --- Start Added Logging ---
                    debug!("Got value as CFNumber, but couldn't convert to i64 or f64.");
                    // --- End Added Logging ---
                }
            } else {
                // Potentially handle other AXValue types (e.g., boolean)
                // --- Start Added Logging ---
                let type_id = value.type_of();
                debug!(
                    "Got AXValue attribute, but it's not CFString or CFNumber. TypeID: {}",
                    type_id
                );
                // --- End Added Logging ---
            }
        } else {
            // --- Start Added Logging ---
            debug!("Failed to get AXValue attribute or it was None.");
            // --- End Added Logging ---
        }
    }

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
                        if !matches!(
                            e,
                            accessibility::Error::Ax(-25212) // attribute unsupported
                                | accessibility::Error::Ax(-25205) // no value
                                | accessibility::Error::Ax(-25204) // getting attribute failed (internal error)
                        ) {
                            // debug!("Error getting property attribute '{}': {:?}", name_str, e);
                        }
                    }
                }
            }
        }
    }
    attrs
}
