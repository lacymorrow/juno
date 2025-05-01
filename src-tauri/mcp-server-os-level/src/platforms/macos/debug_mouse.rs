use core_graphics::display::{CGDisplayBounds, CGMainDisplayID, CGPoint, CGGetActiveDisplayList};
use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use std::time::Duration;

use tracing::info;

/// Get information about all displays
pub fn debug_display_info() {
    // Get the main display ID
    let main_display_id = unsafe { CGMainDisplayID() };
    let main_bounds = unsafe { CGDisplayBounds(main_display_id) };

    info!("MOUSE DEBUG - Main Display: ID={}, bounds=({}, {}, {}, {})",
        main_display_id,
        main_bounds.origin.x, main_bounds.origin.y,
        main_bounds.size.width, main_bounds.size.height);

    // Get all active displays
    let max_displays = 16; // Assuming no more than 16 displays
    let mut display_ids = vec![0; max_displays];
    let mut display_count: u32 = 0;

    let result = unsafe {
        CGGetActiveDisplayList(max_displays as u32, display_ids.as_mut_ptr(), &mut display_count)
    };

    if result != 0 {
        info!("MOUSE DEBUG - Error getting display list: {}", result);
        return;
    }

    // Resize the vector to the actual number of displays
    display_ids.truncate(display_count as usize);

    // Print information about each display
    for (i, &display_id) in display_ids.iter().enumerate() {
        let bounds = unsafe { CGDisplayBounds(display_id) };
        info!("MOUSE DEBUG - Display {}: ID={}, bounds=({}, {}, {}, {})",
            i, display_id,
            bounds.origin.x, bounds.origin.y,
            bounds.size.width, bounds.size.height);
    }
}

/// Detect which display contains the given point
pub fn debug_point_display(x: f64, y: f64) {
    let point = CGPoint::new(x, y);

    // Get all active displays
    let max_displays = 16;
    let mut display_ids = vec![0; max_displays];
    let mut display_count: u32 = 0;

    let result = unsafe {
        CGGetActiveDisplayList(max_displays as u32, display_ids.as_mut_ptr(), &mut display_count)
    };

    if result != 0 {
        info!("MOUSE DEBUG - Error getting display list: {}", result);
        return;
    }

    // Resize the vector to the actual number of displays
    display_ids.truncate(display_count as usize);

    // Check which display contains the point
    let mut found = false;
    for (i, &display_id) in display_ids.iter().enumerate() {
        let bounds = unsafe { CGDisplayBounds(display_id) };

        if point.x >= bounds.origin.x &&
           point.x < bounds.origin.x + bounds.size.width &&
           point.y >= bounds.origin.y &&
           point.y < bounds.origin.y + bounds.size.height {

            info!("MOUSE DEBUG - Point ({}, {}) is on Display {}: ID={}, bounds=({}, {}, {}, {})",
                point.x, point.y, i, display_id,
                bounds.origin.x, bounds.origin.y,
                bounds.size.width, bounds.size.height);

            // Get display-relative coordinates
            let display_x = point.x - bounds.origin.x;
            let display_y = point.y - bounds.origin.y;

            info!("MOUSE DEBUG - Display-relative coordinates: ({}, {})", display_x, display_y);
            found = true;
            break;
        }
    }

    if !found {
        info!("MOUSE DEBUG - Point ({}, {}) is not on any display", point.x, point.y);
    }
}

/// Get the current cursor position and report which display it's on
pub fn debug_cursor_position() -> (f64, f64) {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .expect("Failed to create event source");

    let event = CGEvent::new(source)
        .expect("Failed to create null event for cursor position");

    let point = event.location();

    info!("MOUSE DEBUG - Current cursor position: ({}, {})", point.x, point.y);
    debug_point_display(point.x, point.y);

    (point.x, point.y)
}

/// Test clicking at the given coordinates and report the result
pub fn debug_click_test(x: f64, y: f64) {
    info!("MOUSE DEBUG - Testing click at ({}, {})", x, y);
    debug_point_display(x, y);

    // First, get the current cursor position
    let (current_x, current_y) = debug_cursor_position();

    // Move the mouse to the target position
    let point = CGPoint::new(x, y);
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .expect("Failed to create event source");

    // Move mouse
    let move_event = CGEvent::new_mouse_event(source.clone(), CGEventType::MouseMoved, point, CGMouseButton::Left)
        .expect("Failed to create mouse move event");

    move_event.post(CGEventTapLocation::HID);
    std::thread::sleep(Duration::from_millis(500)); // Give time for the move to complete

    // Get the new cursor position after the move
    let (moved_x, moved_y) = debug_cursor_position();

    // Check if the mouse moved to where we expected
    info!("MOUSE DEBUG - Cursor movement: Expected=({}, {}), Actual=({}, {})",
          x, y, moved_x, moved_y);

    // Now test a click
    info!("MOUSE DEBUG - Clicking at current position: ({}, {})", moved_x, moved_y);

    let click_down = CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseDown, point, CGMouseButton::Left)
        .expect("Failed to create mouse down event");

    let click_up = CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseUp, point, CGMouseButton::Left)
        .expect("Failed to create mouse up event");

    click_down.post(CGEventTapLocation::HID);
    std::thread::sleep(Duration::from_millis(100));
    click_up.post(CGEventTapLocation::HID);

    // Move the cursor back to its original position
    let original_point = CGPoint::new(current_x, current_y);
    let restore_event = CGEvent::new_mouse_event(source, CGEventType::MouseMoved, original_point, CGMouseButton::Left)
        .expect("Failed to create mouse restore event");

    std::thread::sleep(Duration::from_millis(1000)); // Let user see the click result
    restore_event.post(CGEventTapLocation::HID);

    info!("MOUSE DEBUG - Cursor restored to original position: ({}, {})", current_x, current_y);
}
