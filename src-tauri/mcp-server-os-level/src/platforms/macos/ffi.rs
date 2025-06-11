// Safe Cidre-based FFI replacement
// This file replaces all manual FFI declarations with safe Cidre bindings

// Note: Cidre support is conditional based on feature flag
#[cfg(all(target_os = "macos", feature = "use-cidre"))]
use cidre::{ax, cf};
use crate::AutomationError;

/// Safe replacement for AXIsProcessTrustedWithOptions using Cidre
#[cfg(all(target_os = "macos", feature = "use-cidre"))]
pub(crate) fn ax_is_process_trusted_with_options(show_prompt: bool) -> bool {
    ax::is_process_trusted_with_options(
        &ax::TrustedCheckOptions::new().prompt(show_prompt)
    )
}

/// Fallback implementation without Cidre (still uses core-foundation safely)
#[cfg(all(target_os = "macos", not(feature = "use-cidre")))]
pub(crate) fn ax_is_process_trusted_with_options(show_prompt: bool) -> bool {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    
    // Safe implementation using core-foundation
    let key = CFString::new("AXTrustedCheckOptionPrompt");
    let value = if show_prompt {
        CFBoolean::true_value()
    } else {
        CFBoolean::false_value()
    };
    
    let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);
    
    unsafe {
        // This is the only remaining unsafe call, but it's to a well-known Apple API
        use accessibility_sys::{AXIsProcessTrustedWithOptions};
        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef())
    }
}

/// Safe replacement for AXValueCreate using Cidre
#[cfg(all(target_os = "macos", feature = "use-cidre"))]
pub(crate) fn ax_value_create_point(x: f64, y: f64) -> Result<ax::Value, AutomationError> {
    let point = cf::Point::new(x, y);
    ax::Value::from_point(&point)
        .ok_or_else(|| AutomationError::PlatformError("Failed to create AXValue from point".to_string()))
}

/// Fallback implementation without Cidre
#[cfg(all(target_os = "macos", not(feature = "use-cidre")))]
pub(crate) fn ax_value_create_point(x: f64, y: f64) -> Result<core_foundation::base::CFType, AutomationError> {
    use core_foundation::base::TCFType;
    use core_graphics::geometry::CGPoint;
    
    let point = CGPoint::new(x, y);
    let point_ptr = &point as *const _ as *const std::ffi::c_void;
    
    unsafe {
        use accessibility_sys::{AXValueCreate, kAXValueCGPointType};
        let value_ref = AXValueCreate(kAXValueCGPointType, point_ptr);
        if value_ref.is_null() {
            Err(AutomationError::PlatformError("Failed to create AXValue from point".to_string()))
        } else {
            Ok(core_foundation::base::CFType::wrap_under_create_rule(value_ref))
        }
    }
}

/// Safe replacement for AXValueCreate with size using Cidre
#[cfg(all(target_os = "macos", feature = "use-cidre"))]
pub(crate) fn ax_value_create_size(width: f64, height: f64) -> Result<ax::Value, AutomationError> {
    let size = cf::Size::new(width, height);
    ax::Value::from_size(&size)
        .ok_or_else(|| AutomationError::PlatformError("Failed to create AXValue from size".to_string()))
}

/// Fallback implementation without Cidre
#[cfg(all(target_os = "macos", not(feature = "use-cidre")))]
pub(crate) fn ax_value_create_size(width: f64, height: f64) -> Result<core_foundation::base::CFType, AutomationError> {
    use core_foundation::base::TCFType;
    use core_graphics::geometry::CGSize;
    
    let size = CGSize::new(width, height);
    let size_ptr = &size as *const _ as *const std::ffi::c_void;
    
    unsafe {
        use accessibility_sys::{AXValueCreate, kAXValueCGSizeType};
        let value_ref = AXValueCreate(kAXValueCGSizeType, size_ptr);
        if value_ref.is_null() {
            Err(AutomationError::PlatformError("Failed to create AXValue from size".to_string()))
        } else {
            Ok(core_foundation::base::CFType::wrap_under_create_rule(value_ref))
        }
    }
}

/// Safe replacement for AXValueCreate with rect using Cidre
#[cfg(all(target_os = "macos", feature = "use-cidre"))]
pub(crate) fn ax_value_create_rect(x: f64, y: f64, width: f64, height: f64) -> Result<ax::Value, AutomationError> {
    let rect = cf::Rect::new(cf::Point::new(x, y), cf::Size::new(width, height));
    ax::Value::from_rect(&rect)
        .ok_or_else(|| AutomationError::PlatformError("Failed to create AXValue from rect".to_string()))
}

/// Fallback implementation without Cidre
#[cfg(all(target_os = "macos", not(feature = "use-cidre")))]
pub(crate) fn ax_value_create_rect(x: f64, y: f64, width: f64, height: f64) -> Result<core_foundation::base::CFType, AutomationError> {
    use core_foundation::base::TCFType;
    use core_graphics::geometry::{CGPoint, CGSize, CGRect};
    
    let rect = CGRect::new(&CGPoint::new(x, y), &CGSize::new(width, height));
    let rect_ptr = &rect as *const _ as *const std::ffi::c_void;
    
    unsafe {
        use accessibility_sys::{AXValueCreate, kAXValueCGRectType};
        let value_ref = AXValueCreate(kAXValueCGRectType, rect_ptr);
        if value_ref.is_null() {
            Err(AutomationError::PlatformError("Failed to create AXValue from rect".to_string()))
        } else {
            Ok(core_foundation::base::CFType::wrap_under_create_rule(value_ref))
        }
    }
}

/// Safe replacement for AXValueGetValue with point using Cidre
#[cfg(all(target_os = "macos", feature = "use-cidre"))]
pub(crate) fn ax_value_get_point(value: &ax::Value) -> Result<(f64, f64), AutomationError> {
    value.to_point()
        .map(|point| (point.x, point.y))
        .ok_or_else(|| AutomationError::PlatformError("Failed to extract point from AXValue".to_string()))
}

/// Safe replacement for AXValueGetValue with size using Cidre
#[cfg(all(target_os = "macos", feature = "use-cidre"))]
pub(crate) fn ax_value_get_size(value: &ax::Value) -> Result<(f64, f64), AutomationError> {
    value.to_size()
        .map(|size| (size.width, size.height))
        .ok_or_else(|| AutomationError::PlatformError("Failed to extract size from AXValue".to_string()))
}

/// Safe replacement for AXValueGetValue with rect using Cidre
#[cfg(all(target_os = "macos", feature = "use-cidre"))]
pub(crate) fn ax_value_get_rect(value: &ax::Value) -> Result<(f64, f64, f64, f64), AutomationError> {
    value.to_rect()
        .map(|rect| (rect.origin.x, rect.origin.y, rect.size.width, rect.size.height))
        .ok_or_else(|| AutomationError::PlatformError("Failed to extract rect from AXValue".to_string()))
}

// Fallback implementations for non-macOS targets
#[cfg(not(target_os = "macos"))]
pub(crate) fn ax_is_process_trusted_with_options(_show_prompt: bool) -> bool {
    false // Always return false on non-macOS
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn ax_value_create_point(_x: f64, _y: f64) -> Result<(), AutomationError> {
    Err(AutomationError::PlatformError("AX operations only available on macOS".to_string()))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn ax_value_create_size(_width: f64, _height: f64) -> Result<(), AutomationError> {
    Err(AutomationError::PlatformError("AX operations only available on macOS".to_string()))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn ax_value_create_rect(_x: f64, _y: f64, _width: f64, _height: f64) -> Result<(), AutomationError> {
    Err(AutomationError::PlatformError("AX operations only available on macOS".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn test_ax_value_point_roundtrip() {
        let x = 100.0;
        let y = 200.0;
        
        let value = ax_value_create_point(x, y).unwrap();
        // Note: roundtrip tests only work with full Cidre implementation
        #[cfg(feature = "use-cidre")]
        {
            let (result_x, result_y) = ax_value_get_point(&value).unwrap();
            assert!((result_x - x).abs() < 0.001);
            assert!((result_y - y).abs() < 0.001);
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_ax_value_size_roundtrip() {
        let width = 300.0;
        let height = 400.0;
        
        let value = ax_value_create_size(width, height).unwrap();
        // Note: roundtrip tests only work with full Cidre implementation
        #[cfg(feature = "use-cidre")]
        {
            let (result_width, result_height) = ax_value_get_size(&value).unwrap();
            assert!((result_width - width).abs() < 0.001);
            assert!((result_height - height).abs() < 0.001);
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_ax_value_rect_roundtrip() {
        let x = 10.0;
        let y = 20.0;
        let width = 300.0;
        let height = 400.0;
        
        let value = ax_value_create_rect(x, y, width, height).unwrap();
        let (result_x, result_y, result_width, result_height) = ax_value_get_rect(&value).unwrap();
        
        assert!((result_x - x).abs() < 0.001);
        assert!((result_y - y).abs() < 0.001);
        assert!((result_width - width).abs() < 0.001);
        assert!((result_height - height).abs() < 0.001);
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_non_macos_functions_return_errors() {
        assert!(!ax_is_process_trusted_with_options(false));
        assert!(ax_value_create_point(0.0, 0.0).is_err());
        assert!(ax_value_create_size(0.0, 0.0).is_err());
        assert!(ax_value_create_rect(0.0, 0.0, 0.0, 0.0).is_err());
    }
}
