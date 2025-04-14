use super::element::MacOSUIElement;
use super::permissions::check_accessibility_permissions;
use super::utils::{get_pid_for_element, get_running_application_pids, map_generic_role_to_macos_roles, element_contains_text};
use super::wrappers::ThreadSafeAXUIElement;
use crate::{
    AutomationError,
    UIElement,
    Selector,
};
use accessibility::{AXUIElementAttributes, AXAttribute, AXUIElement};
use anyhow::Result;
use core_graphics::display::CGPoint;
use core_graphics::event::{CGEventType, CGMouseButton, CGEventTapLocation, CGEvent};
use core_graphics::event_source::{CGEventSourceStateID, CGEventSource};
use tracing::{debug, trace};
use crate::element::UIElementImpl;
use crate::platforms::AccessibilityEngine;
use crate::platforms::tree_search::{ElementFinderWithWindows, ElementsCollectorWithWindows, TreeWalkerWithWindows};
use std::sync::Arc;
use core_foundation::string::CFString;

pub struct MacOSEngine {
    pub(crate) system_wide: ThreadSafeAXUIElement,
    pub(crate) use_background_apps: bool,
    pub(crate) activate_app: bool,
}

impl MacOSEngine {
    pub fn new(use_background_apps: bool, activate_app: bool) -> Result<Self, AutomationError> {
        check_accessibility_permissions(false)?;

        Ok(Self {
            system_wide: ThreadSafeAXUIElement::system_wide(),
            use_background_apps,
            activate_app,
        })
    }

    pub(crate) fn wrap_element(&self, ax_element: ThreadSafeAXUIElement) -> UIElement {
        let is_valid = match ax_element.0.role() {
            Ok(_) => true,
            Err(e) => {
                debug!("Warning: Potentially invalid AXUIElement: {:?}", e);
                false
            }
        };

        if !is_valid {
            debug!("Warning: Wrapping possibly invalid AXUIElement");
        }

        UIElement::new(Box::new(MacOSUIElement {
            element: ax_element,
            use_background_apps: self.use_background_apps,
            activate_app: self.activate_app,
        }))
    }

    #[allow(clippy::unexpected_cfg_condition)]
    pub(crate) fn refresh_accessibility_tree(
        &self,
        app_name: Option<&str>,
    ) -> Result<(), AutomationError> {
        if !self.activate_app {
            return Ok(());
        }

        debug!("Refreshing accessibility tree");

        if let Some(name) = app_name {
            unsafe {
                use objc::{class, msg_send, sel, sel_impl};

                let workspace_class = class!(NSWorkspace);
                let shared_workspace: *mut objc::runtime::Object =
                    msg_send![workspace_class, sharedWorkspace];
                let apps: *mut objc::runtime::Object =
                    msg_send![shared_workspace, runningApplications];
                let count: usize = msg_send![apps, count];

                for i in 0..count {
                    let app: *mut objc::runtime::Object = msg_send![apps, objectAtIndex:i];
                    let app_name_obj: *mut objc::runtime::Object = msg_send![app, localizedName];

                    if !app_name_obj.is_null() {
                        let app_name_str: &str = {
                            let nsstring = app_name_obj as *const objc::runtime::Object;
                            let bytes: *const std::os::raw::c_char =
                                msg_send![nsstring, UTF8String];
                            let len: usize = msg_send![nsstring, lengthOfBytesUsingEncoding:4];
                            let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
                            std::str::from_utf8_unchecked(bytes_slice)
                        };

                        if app_name_str.to_lowercase() == name.to_lowercase() {
                            let _: () = msg_send![app, activateWithOptions:1];
                            debug!("Activated application: {}", name);

                            std::thread::sleep(std::time::Duration::from_millis(100));
                            break;
                        }
                    }
                }
            }
        }

        let _ = self.system_wide.0.attribute_names();

        Ok(())
    }

    pub(crate) fn focus_application_with_cache(
        &self,
        app_name: &str,
        app_cache: Option<&ThreadSafeAXUIElement>,
    ) -> Result<ThreadSafeAXUIElement, AutomationError> {
        debug!("focusing application: {}", app_name);

        if let Some(cached_element) = app_cache {
            debug!("using cached application element");

            match cached_element.0.role() {
                Ok(role) if role.to_string() == "AXApplication" => {
                    unsafe {
                        use objc::{class, msg_send, sel, sel_impl};
                        let pid = get_pid_for_element(cached_element);

                        let nsra_class = class!(NSRunningApplication);
                        let app: *mut objc::runtime::Object =
                            msg_send![nsra_class, runningApplicationWithProcessIdentifier:pid];
                        if !app.is_null() {
                            let _: () = msg_send![app, activateWithOptions:1];
                            debug!("Activated application using cached element");

                            return Ok(cached_element.clone());
                        }
                    }
                }
                _ => {
                    debug!("Cached element is no longer valid");
                }
            }
        }

        self.refresh_accessibility_tree(Some(app_name))?;

        unsafe {
            use objc::{class, msg_send, sel, sel_impl};

            let workspace_class = class!(NSWorkspace);
            let shared_workspace: *mut objc::runtime::Object =
                msg_send![workspace_class, sharedWorkspace];
            let apps: *mut objc::runtime::Object = msg_send![shared_workspace, runningApplications];
            let count: usize = msg_send![apps, count];

            for i in 0..count {
                let app: *mut objc::runtime::Object = msg_send![apps, objectAtIndex:i];
                let app_name_obj: *mut objc::runtime::Object = msg_send![app, localizedName];

                if !app_name_obj.is_null() {
                    let app_name_str: &str = {
                        let nsstring = app_name_obj as *const objc::runtime::Object;
                        let bytes: *const std::os::raw::c_char = msg_send![nsstring, UTF8String];
                        let len: usize = msg_send![nsstring, lengthOfBytesUsingEncoding:4];
                        let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
                        std::str::from_utf8_unchecked(bytes_slice)
                    };

                    if app_name_str.to_lowercase() == app_name.to_lowercase() {
                        let pid: i32 = msg_send![app, processIdentifier];
                        let ax_element = ThreadSafeAXUIElement::application(pid);

                        return Ok(ax_element);
                    }
                }
            }
        }

        Err(AutomationError::ElementNotFound(format!(
            "Application '{}' not found",
            app_name
        )))
    }

    pub(crate) fn scroll_at_position(
        &self,
        x: f64,
        y: f64,
        direction: &str,
        amount: f64,
    ) -> Result<(), AutomationError> {
        debug!("scrolling {} by {} at position ({}, {})", direction, amount, x, y);

        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| {
                AutomationError::PlatformError("Failed to create event source".to_string())
            })?;

        let scroll_amount = amount as i32;

        let (scroll_x, scroll_y) = match direction.to_lowercase().as_str() {
            "up" => (0, -scroll_amount),
            "down" => (0, scroll_amount),
            "left" => (-scroll_amount, 0),
            "right" => (scroll_amount, 0),
            _ => {
                return Err(AutomationError::InvalidArgument(format!(
                    "Invalid scroll direction: {}. Must be up, down, left, or right",
                    direction
                )))
            }
        };

        let point = CGPoint::new(x, y);
        let mouse_move = CGEvent::new_mouse_event(
            source.clone(),
            CGEventType::MouseMoved,
            point,
            CGMouseButton::Left,
        )
        .map_err(|_| {
            AutomationError::PlatformError("Failed to create mouse move event".to_string())
        })?;
        mouse_move.post(CGEventTapLocation::HID);

        std::thread::sleep(std::time::Duration::from_millis(50));

        let scroll_event = CGEvent::new_scroll_event(
            source, 0, 1,
            scroll_y, scroll_x, 0,
        )
        .map_err(|_| AutomationError::PlatformError("Failed to create scroll event".to_string()))?;

        scroll_event.post(CGEventTapLocation::HID);

        debug!("scrolled {} by {} at position ({}, {})", direction, amount, x, y);
        Ok(())
    }

    pub(crate) fn scroll_at_current_position(
        &self,
        direction: &str,
        amount: f64,
    ) -> Result<(), AutomationError> {
        debug!("getting current mouse location using CGEvent::new with a valid event source");

        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| AutomationError::PlatformError("failed to create event source".to_string()))?;
        debug!("created event source successfully");

        let event = CGEvent::new(source)
            .map_err(|_| AutomationError::PlatformError("failed to create event for obtaining current mouse position".to_string()))?;
        debug!("got current event; mouse position: {:?}", event.location());

        let current_pos = event.location();

        self.scroll_at_position(current_pos.x, current_pos.y, direction, amount)
    }

}

impl AccessibilityEngine for MacOSEngine {
    fn get_applications(&self) -> Result<Vec<UIElement>, AutomationError> {
        // Get running application PIDs using NSWorkspace
        let pids = get_running_application_pids(self.use_background_apps)?;

        debug!("Found {} running applications", pids.len());

        // Create AXUIElements for each application
        let mut app_elements = Vec::new();
        for pid in pids {
            trace!("Creating AXUIElement for application with PID: {}", pid);
            let app_element = ThreadSafeAXUIElement::application(pid);

            app_elements.push(self.wrap_element(app_element));
        }

        Ok(app_elements)
    }
    fn get_root_element(&self) -> UIElement {
        self.wrap_element(self.system_wide.clone())
    }

    fn get_focused_element(&self) -> Result<UIElement, AutomationError> {
        // not implemented
        Err(AutomationError::UnsupportedOperation(
            "get_focused_element not yet implemented for macOS".to_string(),
        ))
    }

    fn get_application_by_name(&self, name: &str) -> Result<UIElement, AutomationError> {
        // Refresh the accessibility tree before searching
        self.refresh_accessibility_tree(Some(name))?;

        // Get all applications first, then filter by name
        let apps = self.get_applications()?;

        debug!(
            "Searching for application '{}' among {} applications",
            name,
            apps.len()
        );

        // Optimization: Convert target name to lowercase once, outside the loop
        let name_lowercase = name.to_lowercase();

        // Look for an application with a matching name
        for app in apps {
            let app_name = app.attributes().label.unwrap_or_default();
            // debug!("Checking application: '{}'", app_name);
            // std::thread::sleep(std::time::Duration::from_millis(1));

            // Case-insensitive comparison with pre-computed lowercase name
            if app_name.to_lowercase() == name_lowercase {
                debug!("found matching application: '{}'", app_name);
                return Ok(app);
            }
        }

        // No matching application found
        Err(AutomationError::ElementNotFound(format!(
            "Application '{}' not found",
            name
        )))
    }

    fn find_element(
        &self,
        selector: &Selector,
        root: Option<&UIElement>,
    ) -> Result<UIElement, AutomationError> {
        // If we have a root element that's an application, refresh the tree for that app
        if let Some(root_elem) = root {
            if let Some(macos_el) = root_elem.as_any().downcast_ref::<MacOSUIElement>() {
                if macos_el
                    .element
                    .0
                    .role()
                    .map_or(false, |r| r.to_string() == "AXApplication")
                {
                    if let Some(app_name) = root_elem.attributes().label {
                        self.refresh_accessibility_tree(Some(&app_name))?;
                    }
                }
            }
        }

        let start_element = root
            .map(|el| {
                if let Some(macos_el) = el.as_any().downcast_ref::<MacOSUIElement>() {
                    &macos_el.element.0
                } else {
                    panic!("Root element is not a macOS element")
                }
            })
            .unwrap_or(&self.system_wide.0);

        // Regular element finding logic
        match selector {
            Selector::Role { role, name: _ } => {
                // Get all possible macOS roles for this generic role
                let macos_roles = map_generic_role_to_macos_roles(role);

                let collector = ElementFinderWithWindows::new(
                    &self.system_wide.0,
                    move |e| {
                        let element_role = e.role().unwrap_or(CFString::new("")).to_string();
                        macos_roles.contains(&element_role)
                    },
                    None,
                );
                let walker: TreeWalkerWithWindows = TreeWalkerWithWindows::new();

                walker.walk(start_element, &collector);

                let ax_ui_element = match collector.find() {
                    Ok(ax_ui_element) => ax_ui_element,
                    Err(_) => {
                        return Err(AutomationError::ElementNotFound(format!(
                            "Element with role '{}' not found",
                            role
                        )))
                    }
                };
                Ok(self.wrap_element(ThreadSafeAXUIElement::new(ax_ui_element)))
            }
            Selector::Id(id) => {
                let id_owned = id.clone(); // Create an owned copy
                let collector = ElementFinderWithWindows::new(
                    &self.system_wide.0,
                    move |e| {
                        // Use move to take ownership of id_owned
                        e.identifier().unwrap_or(CFString::new("")).to_string() == id_owned
                    },
                    None,
                );
                let walker: TreeWalkerWithWindows = TreeWalkerWithWindows::new();

                walker.walk(start_element, &collector);

                let ax_ui_element = match collector.find() {
                    Ok(ax_ui_element) => ax_ui_element,
                    Err(_) => {
                        return Err(AutomationError::ElementNotFound(format!(
                            "Element with ID '{}' not found",
                            id
                        )))
                    }
                };
                Ok(self.wrap_element(ThreadSafeAXUIElement::new(ax_ui_element)))
            }
            Selector::Name(name) => {
                let name_owned = name.clone(); // Create an owned copy
                let collector = ElementFinderWithWindows::new(
                    &self.system_wide.0,
                    move |e| {
                        // Use move to take ownership of name_owned
                        e.title().unwrap_or(CFString::new("")).to_string() == name_owned
                    },
                    None,
                );
                let walker: TreeWalkerWithWindows = TreeWalkerWithWindows::new();

                walker.walk(start_element, &collector);

                let ax_ui_element = match collector.find() {
                    Ok(ax_ui_element) => ax_ui_element,
                    Err(_) => {
                        return Err(AutomationError::ElementNotFound(format!(
                            "Element with name '{}' not found",
                            name
                        )))
                    }
                };
                Ok(self.wrap_element(ThreadSafeAXUIElement::new(ax_ui_element)))
            }

            Selector::Text(text) => {
                let text_owned = text.clone(); // Create an owned copy

                // Create a collector that recursively checks children
                let collector = ElementFinderWithWindows::new(
                    &self.system_wide.0,
                    move |e| {
                        // First check if element itself contains the text in any attribute
                        if element_contains_text(e, &text_owned) {
                            return true;
                        }

                        false
                    },
                    None,
                );

                let walker: TreeWalkerWithWindows = TreeWalkerWithWindows::new();

                walker.walk(start_element, &collector);

                let ax_ui_element = match collector.find() {
                    Ok(ax_ui_element) => ax_ui_element,
                    Err(_) => {
                        return Err(AutomationError::ElementNotFound(format!(
                            "Element with text '{}' not found",
                            text
                        )))
                    }
                };
                Ok(self.wrap_element(ThreadSafeAXUIElement::new(ax_ui_element)))
            }
            Selector::Attributes(_attrs) => Err(AutomationError::UnsupportedOperation(
                "Attributes selector not implemented".to_string(),
            )),
            Selector::Path(_) => Err(AutomationError::UnsupportedOperation(
                "Path selector not implemented".to_string(),
            )),
            Selector::Chain(selectors) => {
                // For now, only support role -> id pattern
                if selectors.len() != 2 {
                    return Err(AutomationError::UnsupportedOperation(
                        "Only role -> id chains are supported".to_string(),
                    ));
                }

                // Check if it's a role -> id pattern
                if let (Selector::Role { role, name: _ }, Selector::Id(id)) =
                    (&selectors[0], &selectors[1])
                {
                    debug!("processing chain: role '{}' -> id '{}'", role, id);

                    // First find elements matching the role
                    let role_elements = self.find_elements(&selectors[0], root)?;
                    debug!(
                        "found {} elements matching role '{}'",
                        role_elements.len(),
                        role
                    );

                    // Then find the one with matching id
                    for element in role_elements {
                        if let Some(element_id) = element.id() {
                            if element_id == *id {
                                debug!("found matching element with id '{}'", id);
                                return Ok(element);
                            }
                        }
                    }

                    return Err(AutomationError::ElementNotFound(format!(
                        "no element found with role '{}' and id '{}'",
                        role, id
                    )));
                } else {
                    return Err(AutomationError::UnsupportedOperation(
                        "only role -> id chains are supported".to_string(),
                    ));
                }
            },
            Selector::Filter(_) => Err(AutomationError::UnsupportedOperation(
                "Filter selector not implemented".to_string(),
            )),
        }
    }

    fn find_elements(
        &self,
        selector: &Selector,
        root: Option<&UIElement>,
    ) -> Result<Vec<UIElement>, AutomationError> {
        // Get the start element from the provided root or fall back to system_wide
        let start_element = root
            .map(|el| {
                if let Some(macos_el) = el.as_any().downcast_ref::<MacOSUIElement>() {
                    &macos_el.element.0
                } else {
                    panic!("Root element is not a macOS element")
                }
            })
            .unwrap_or(&self.system_wide.0);

        match selector {
            Selector::Role { role, name: _ } => {
                let macos_roles = map_generic_role_to_macos_roles(role);

                let collector = ElementsCollectorWithWindows::new(start_element, move |e| {
                    let element_role = e.role().unwrap_or(CFString::new("")).to_string();
                    macos_roles.contains(&element_role)
                });

                let ax_ui_elements = collector.find_all();

                // Convert AXUIElements to UIElements
                let ui_elements = ax_ui_elements
                    .into_iter()
                    .map(|e| self.wrap_element(ThreadSafeAXUIElement::new(e)))
                    .collect();

                Ok(ui_elements)
            }
            Selector::Id(id) => {
                let id_owned = id.clone();
                let collector = ElementsCollectorWithWindows::new(start_element, move |e| {
                    e.identifier().unwrap_or(CFString::new("")).to_string() == id_owned
                });

                let ax_ui_elements = collector.find_all();

                // Convert AXUIElements to UIElements
                let ui_elements = ax_ui_elements
                    .into_iter()
                    .map(|e| self.wrap_element(ThreadSafeAXUIElement::new(e)))
                    .collect();

                Ok(ui_elements)
            }
            Selector::Name(name) => {
                let name_owned = name.clone();
                let collector = ElementsCollectorWithWindows::new(start_element, move |e| {
                    e.title().unwrap_or(CFString::new("")).to_string() == name_owned
                });

                let ax_ui_elements = collector.find_all();

                // Convert AXUIElements to UIElements
                let ui_elements = ax_ui_elements
                    .into_iter()
                    .map(|e| self.wrap_element(ThreadSafeAXUIElement::new(e)))
                    .collect();

                Ok(ui_elements)
            }
            Selector::Text(text) => {
                let text_owned = text.clone();
                let collector = ElementsCollectorWithWindows::new(start_element, move |e| {
                    element_contains_text(e, &text_owned)
                });

                let ax_ui_elements = collector.find_all();

                // Convert AXUIElements to UIElements
                let ui_elements = ax_ui_elements
                    .into_iter()
                    .map(|e| self.wrap_element(ThreadSafeAXUIElement::new(e)))
                    .collect();

                Ok(ui_elements)
            }
            Selector::Attributes(_attrs) => Err(AutomationError::UnsupportedOperation(
                "Attributes selector not implemented for find_elements".to_string(),
            )),
            Selector::Path(_) => Err(AutomationError::UnsupportedOperation(
                "Path selector not implemented for find_elements".to_string(),
            )),
            Selector::Filter(_) => Err(AutomationError::UnsupportedOperation(
                "Filter selector not implemented for find_elements".to_string(),
            )),
            Selector::Chain(_) => Err(AutomationError::UnsupportedOperation(
                "Chain selector not implemented for find_elements".to_string(),
            )),
        }
    }

    fn open_application(&self, app_name: &str) -> Result<UIElement, AutomationError> {
        debug!("opening application: {}", app_name);

        // Launch the application
        let status = std::process::Command::new("open")
            .args(["-a", app_name])
            .status()
            .map_err(|e| {
                AutomationError::PlatformError(format!("failed to execute 'open' command: {}", e))
            })?;

        if !status.success() {
            return Err(AutomationError::PlatformError(format!(
                "failed to open application '{}': exit code {:?}",
                app_name,
                status.code()
            )));
        }

        // Use a more efficient approach - directly get the app PID without full system scan
        let mut retry_count = 0;
        let max_retries = 10;
        let retry_delay = std::time::Duration::from_millis(100);

        // Retry loop with targeted scanning
        while retry_count < max_retries {
            debug!("looking for newly launched app '{}', attempt {}/{}",
                   app_name, retry_count + 1, max_retries);

            // Try to find the app directly without full refresh
            unsafe {
                use objc::{class, msg_send, sel, sel_impl};

                let workspace_class = class!(NSWorkspace);
                let shared_workspace: *mut objc::runtime::Object =
                    msg_send![workspace_class, sharedWorkspace];
                let apps: *mut objc::runtime::Object =
                    msg_send![shared_workspace, runningApplications];
                let count: usize = msg_send![apps, count];

                for i in 0..count {
                    let app: *mut objc::runtime::Object = msg_send![apps, objectAtIndex:i];
                    let app_name_obj: *mut objc::runtime::Object = msg_send![app, localizedName];

                    if !app_name_obj.is_null() {
                        let found_name: &str = {
                            let nsstring = app_name_obj as *const objc::runtime::Object;
                            let bytes: *const std::os::raw::c_char =
                                msg_send![nsstring, UTF8String];
                            let len: usize = msg_send![nsstring, lengthOfBytesUsingEncoding:4];
                            let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
                            std::str::from_utf8_unchecked(bytes_slice)
                        };

                        if found_name.to_lowercase() == app_name.to_lowercase() {
                            // Found the app, get its PID and create element directly
                            let pid: i32 = msg_send![app, processIdentifier];
                            debug!("found newly launched app '{}' with pid {}", app_name, pid);

                            // Create element directly instead of full scan
                            let app_element = ThreadSafeAXUIElement::application(pid);
                            return Ok(self.wrap_element(app_element));
                        }
                    }
                }
            }

            // App not found yet, sleep and retry
            std::thread::sleep(retry_delay);
            retry_count += 1;
        }

        // Fallback to existing approach if retries fail
        debug!("retries exceeded, falling back to standard method");
        self.refresh_accessibility_tree(Some(app_name))?;
        self.get_application_by_name(app_name)
    }

    fn open_url(&self, url: &str, browser: Option<&str>) -> Result<UIElement, AutomationError> {
        debug!("opening url: {} in browser: {:?}", url, browser);

        let status = match browser {
            Some(browser_name) => {
                // Open URL in the specified browser
                std::process::Command::new("open")
                    .args(["-a", browser_name, url])
                    .status()
                    .map_err(|e| {
                        AutomationError::PlatformError(format!(
                            "failed to execute 'open' command: {}",
                            e
                        ))
                    })?
            }
            None => {
                // Open URL in the default browser
                std::process::Command::new("open")
                    .arg(url)
                    .status()
                    .map_err(|e| {
                        AutomationError::PlatformError(format!(
                            "failed to execute 'open' command: {}",
                            e
                        ))
                    })?
            }
        };

        if !status.success() {
            return Err(AutomationError::PlatformError(format!(
                "failed to open url '{}': exit code {:?}",
                url,
                status.code()
            )));
        }

        // Give the browser a moment to launch
        std::thread::sleep(std::time::Duration::from_millis(1000));

        // If a specific browser was requested, try to get its UI element
        if let Some(browser_name) = browser {
            // Refresh accessibility tree with the browser
            self.refresh_accessibility_tree(Some(browser_name))?;

            // Get the browser application element
            self.get_application_by_name(browser_name)
        } else {
            // Without a specific browser name, we can't reliably return the browser element
            // Just return the system-wide element
            Ok(self.get_root_element())
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    // fn scroll_at_position(&self, x: f64, y: f64, direction: &str, amount: f64) -> Result<(), AutomationError> {
    //     // Call the struct implementation directly
    //     self.scroll_at_position(x, y, direction, amount)
    // }

    // fn scroll_at_current_position(&self, direction: &str, amount: f64) -> Result<(), AutomationError> {
    //     // Call the struct implementation directly
    //     self.scroll_at_current_position(direction, amount)
    // }
}
