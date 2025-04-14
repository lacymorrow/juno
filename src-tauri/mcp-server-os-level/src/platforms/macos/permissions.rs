use crate::AutomationError;
use crate::platforms::macos::ffi::AXIsProcessTrustedWithOptions; // Import from ffi module
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use core_foundation::base::TCFType;
use tracing::debug;

// Add this new function to the file (not inside any impl block)
pub(crate) fn check_accessibility_permissions(show_prompt: bool) -> Result<bool, AutomationError> {
    debug!("checking accessibility permissions");

    unsafe {
        // Create the options dictionary more safely
        let key = CFString::new("AXTrustedCheckOptionPrompt");
        let value = if show_prompt {
            CFBoolean::true_value()
        } else {
            CFBoolean::false_value()
        };

        // Create dictionary with proper memory management
        let options = CFDictionary::from_CFType_pairs(&[(
            key.as_CFType(),
            value.as_CFType(),
        )]);

        // Call the function with proper type conversion
        let is_trusted = AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef());

        if is_trusted {
            debug!("accessibility permissions are granted");
            Ok(true)
        } else {
            if !show_prompt {
                debug!("accessibility permissions not granted");
                Err(AutomationError::PermissionDenied(
                    "Accessibility permissions not granted. Go to System Preferences > Security & Privacy > Privacy > Accessibility and add this application.".to_string(),
                ))
            } else {
                debug!("accessibility permissions prompt displayed");
                Ok(false)
            }
        }
    }
}
