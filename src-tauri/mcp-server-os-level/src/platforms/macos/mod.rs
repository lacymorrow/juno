pub mod attributes;
pub mod constants;
pub mod element;
pub mod engine;
pub mod ffi;
pub mod interaction;
pub mod input;
pub mod permissions;
pub mod utils;
pub mod wrappers;

// Cidre-based implementations (conditionally compiled)
#[cfg(target_os = "macos")]
pub mod permissions_cidre;
#[cfg(target_os = "macos")]
pub mod utils_cidre;

// Re-export based on feature flags for gradual migration
#[cfg(all(target_os = "macos", feature = "use-cidre"))]
pub use permissions_cidre::{
    check_accessibility_permissions_cidre as check_accessibility_permissions_safe,
    check_accessibility_permissions_with_auto_redirect_cidre as check_accessibility_permissions_with_auto_redirect_safe,
};

#[cfg(all(target_os = "macos", feature = "use-cidre"))]
pub use utils_cidre::{
    get_running_application_pids_cidre as get_running_application_pids_safe,
    get_frontmost_application_cidre as get_frontmost_application_safe,
    get_application_info_by_pid_cidre as get_application_info_by_pid_safe,
    launch_application_cidre as launch_application_safe,
    hide_application_cidre as hide_application_safe,
    activate_application_cidre as activate_application_safe,
    get_display_bounds_cidre as get_display_bounds_safe,
    find_display_containing_point_cidre as find_display_containing_point_safe,
};

// Default exports (legacy implementations)
#[cfg(not(all(target_os = "macos", feature = "use-cidre")))]
pub use permissions::{
    check_accessibility_permissions as check_accessibility_permissions_safe,
    check_accessibility_permissions_with_auto_redirect as check_accessibility_permissions_with_auto_redirect_safe,
};

#[cfg(not(all(target_os = "macos", feature = "use-cidre")))]
pub use utils::{
    get_running_application_pids as get_running_application_pids_safe,
    // Note: Need to add these functions to the original utils.rs if not present
    // get_frontmost_application as get_frontmost_application_safe,
    // get_application_info_by_pid as get_application_info_by_pid_safe,
    // launch_application as launch_application_safe,
    // hide_application as hide_application_safe,
    // activate_application as activate_application_safe,
    // get_display_bounds as get_display_bounds_safe,
    // find_display_containing_point as find_display_containing_point_safe,
};

// No platform-level imports needed here after refactoring

// Re-export key types publicly
pub use element::MacOSUIElement;
pub use engine::MacOSEngine;

// The rest of the original file content has been moved to the respective modules.
