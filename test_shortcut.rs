#!/usr/bin/env rust-script
//! Test script for input monitoring permission check
//! 
//! Run with: rustc test_shortcut.rs && ./test_shortcut

use std::process::Command;

fn check_input_monitoring_permission() -> bool {
    // Try using sqlite3 to check TCC database
    if let Ok(output) = Command::new("sqlite3")
        .args(&[
            &format!("{}/Library/Application Support/com.apple.TCC/TCC.db", 
                    std::env::var("HOME").unwrap_or_else(|_| "/Users/unknown".to_string())),
            "SELECT allowed FROM access WHERE service='kTCCServiceListenEvent' AND (client='com.juno.app' OR client LIKE '%Terminal%');"
        ])
        .output()
    {
        if output.status.success() {
            let result = String::from_utf8_lossy(&output.stdout);
            if result.trim() == "1" {
                println!("✅ Input monitoring granted (TCC check)");
                return true;
            } else if result.trim() == "0" {
                println!("❌ Input monitoring denied (TCC check)");
                return false;
            }
        }
    }
    
    // Try AppleScript test
    if let Ok(output) = Command::new("osascript")
        .args(&[
            "-e",
            r#"try
                tell application "System Events"
                    key code 0
                end tell
                return "true"
            on error
                return "false"
            end try"#
        ])
        .output()
    {
        if output.status.success() {
            let result = String::from_utf8_lossy(&output.stdout);
            let has_permission = result.trim() == "true";
            if has_permission {
                println!("✅ Input monitoring granted (AppleScript test)");
            } else {
                println!("❌ Input monitoring not granted (AppleScript test)");
            }
            return has_permission;
        }
    }
    
    println!("⚠️  Unable to check input monitoring permission");
    false
}

fn main() {
    println!("Testing Input Monitoring Permission Check");
    println!("=========================================");
    
    let has_permission = check_input_monitoring_permission();
    
    println!("\nResult: {}", if has_permission {
        "Input monitoring permission IS granted ✅"
    } else {
        "Input monitoring permission NOT granted ❌"
    });
    
    if !has_permission {
        println!("\nTo grant permission:");
        println!("1. Open System Settings > Privacy & Security > Input Monitoring");
        println!("2. Enable the toggle for your application");
    }
}