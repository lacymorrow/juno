// Standalone program to test macOS coordinate systems for mouse clicking

use computer_use_ai_sdk::platforms::macos::debug_mouse;
use std::io::{self, Write};
use tracing_subscriber::{fmt, EnvFilter};

fn clear_screen() {
    // Basic ANSI escape codes for clearing the screen and moving cursor to top-left
    print!("\x1B[2J\x1B[1;1H");
    // Ensure the buffer is flushed so the clear happens immediately
    io::stdout().flush().unwrap();
}

fn main() {
    // Initialize tracing subscriber
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info")) // Default to info level if RUST_LOG not set
        .unwrap();
    fmt().with_env_filter(filter).init();

    tracing::info!("Starting Mouse Coordinate System Debugging Tool...");

    println!("Mouse Coordinate System Debugging Tool");
    println!("=====================================");
    println!("This tool will help diagnose issues with mouse coordinate systems.");
    println!("Check the console output where this program is running for detailed MOUSE DEBUG logs.");

    // Show initial display information
    println!("\nInitial Display Information:");
    debug_mouse::debug_display_info();

    // Get initial cursor position
    println!("\nInitial Cursor Position:");
    let (x, y) = debug_mouse::debug_cursor_position();
    println!("Current cursor reported at global coordinates: ({:.2}, {:.2})", x, y);

    // Main testing loop
    loop {
        println!("\n---------------------");
        println!("Options:");
        println!("  1. Show all display information");
        println!("  2. Show current cursor position and its display");
        println!("  3. Check which display contains a specific point");
        println!("  4. Test moving mouse and clicking at a specific point");
        println!("  5. Exit");
        print!("\nEnter your choice: ");
        io::stdout().flush().unwrap(); // Ensure prompt is shown before waiting for input

        let mut choice = String::new();
        match io::stdin().read_line(&mut choice) {
            Ok(_) => { /* Proceed */ }
            Err(e) => {
                eprintln!("Error reading input: {}. Exiting.", e);
                break;
            }
        }

        match choice.trim() {
            "1" => {
                clear_screen();
                println!("Display Information:");
                debug_mouse::debug_display_info();
            }
            "2" => {
                clear_screen();
                println!("Current Cursor Position:");
                let (x, y) = debug_mouse::debug_cursor_position();
                println!("Current cursor reported at global coordinates: ({:.2}, {:.2})", x, y);
                println!("(Check logs for display details)");
            }
            "3" => {
                println!("Enter global X coordinate to test:");
                let x = read_f64_input();

                println!("Enter global Y coordinate to test:");
                let y = read_f64_input();

                clear_screen();
                println!("Testing point ({:.2}, {:.2}):", x, y);
                debug_mouse::debug_point_display(x, y);
                println!("(Check logs for display details)");
            }
            "4" => {
                println!("Enter global X coordinate to click:");
                let x = read_f64_input();

                println!("Enter global Y coordinate to click:");
                let y = read_f64_input();

                clear_screen();
                println!("--- Running Click Test at ({:.2}, {:.2}) ---", x, y);
                println!("Watch your cursor move, click, and return.");
                debug_mouse::debug_click_test(x, y);
                println!("Click test finished. Check logs for details.");
            }
            "5" => {
                println!("Exiting...");
                break;
            }
            _ => {
                println!("Invalid choice '{}', please try again.", choice.trim());
            }
        }
    }
}

// Helper function to read f64 input from stdin
fn read_f64_input() -> f64 {
    loop {
        print!("> ");
        io::stdout().flush().unwrap();
        let mut input_str = String::new();
        match io::stdin().read_line(&mut input_str) {
            Ok(_) => match input_str.trim().parse::<f64>() {
                Ok(val) => return val,
                Err(_) => println!("Invalid input. Please enter a number (e.g., 100.0 or 50)."),
            },
            Err(e) => {
                println!("Error reading input: {}. Please try again.", e);
            }
        }
    }
}
