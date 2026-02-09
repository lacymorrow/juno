// Centralized key parsing utilities to reduce redundancy across different keyboard handlers

use tracing::debug;

/// Represents a parsed key combination
#[derive(Debug, Clone)]
pub struct ParsedKeyCombo {
    /// The main key (last part after '+')
    pub key: String,
    /// List of modifier keys (all parts before the last '+')
    pub modifiers: Vec<String>,
}

/// Parse a key combination string (e.g., "cmd+shift+a") into key and modifiers
pub fn parse_key_combination(key_combo: &str) -> Result<ParsedKeyCombo, String> {
    if key_combo.trim().is_empty() {
        return Err("Empty key combination".to_string());
    }

    let parts: Vec<String> = key_combo
        .split('+')
        .map(|s| s.trim().to_lowercase())
        .collect();

    if parts.is_empty() {
        return Err("Invalid key combination format".to_string());
    }

    let key = parts[parts.len() - 1].clone();
    let modifiers = parts[0..parts.len() - 1].to_vec();

    debug!("Parsed key combo '{}' -> key: '{}', modifiers: {:?}", key_combo, key, modifiers);

    Ok(ParsedKeyCombo { key, modifiers })
}

/// Normalize modifier names to standard format
pub fn normalize_modifier(modifier: &str) -> Option<&'static str> {
    match modifier.to_lowercase().as_str() {
        "cmd" | "command" => Some("cmd"),
        "shift" => Some("shift"),
        "alt" | "option" => Some("option"),
        "ctrl" | "control" => Some("control"),
        "fn" => Some("fn"),
        _ => None,
    }
}

/// Validate that all modifiers in a key combination are recognized
pub fn validate_modifiers(modifiers: &[String]) -> Result<Vec<String>, String> {
    let mut normalized = Vec::new();
    for modifier in modifiers {
        match normalize_modifier(modifier) {
            Some(norm) => normalized.push(norm.to_string()),
            None => return Err(format!("Unknown modifier: {}", modifier)),
        }
    }
    Ok(normalized)
}

/// Convert key combination to separate key and modifier for APIs requiring separate values
pub fn split_key_and_modifier(key_combo: &str) -> Result<(String, Option<String>), String> {
    let parsed = parse_key_combination(key_combo)?;

    if parsed.modifiers.is_empty() {
        Ok((parsed.key, None))
    } else if parsed.modifiers.len() == 1 {
        let normalized = validate_modifiers(&parsed.modifiers)?;
        Ok((parsed.key, Some(normalized[0].clone())))
    } else {
        // For multiple modifiers, join them with '+' for APIs that support it
        let normalized = validate_modifiers(&parsed.modifiers)?;
        Ok((parsed.key, Some(normalized.join("+"))))
    }
}

/// Generate AppleScript key combination format
pub fn to_applescript_format(key_combo: &str) -> Result<String, String> {
    let parsed = parse_key_combination(key_combo)?;
    let normalized_modifiers = validate_modifiers(&parsed.modifiers)?;

    // Convert main key to AppleScript format
    let apple_key = match parsed.key.as_str() {
        "return" | "enter" => "return",
        "tab" => "tab",
        "escape" | "esc" => "escape",
        "backspace" | "delete" => "delete",
        "space" => "space",
        "down" | "downarrow" => "down arrow",
        "up" | "uparrow" => "up arrow",
        "left" | "leftarrow" => "left arrow",
        "right" | "rightarrow" => "right arrow",
        _ => &parsed.key,
    };

    let mut script = String::from("tell application \"System Events\" to ");

    // Simple key without modifiers
    if normalized_modifiers.is_empty() && apple_key.len() == 1 {
        script.push_str(&format!("keystroke \"{}\"", apple_key));
        return Ok(script);
    }

    // Key combination with modifiers
    script.push_str("key code ");

    // Map key to AppleScript key code
    let key_code = match apple_key {
        "return" => "36",
        "tab" => "48",
        "escape" => "53",
        "delete" => "51",
        "space" => "49",
        "down arrow" => "125",
        "up arrow" => "126",
        "left arrow" => "123",
        "right arrow" => "124",
        _ => {
            // For single character keys, use a simplified mapping
            if apple_key.len() == 1 {
                let c = match apple_key.chars().next() {
                    Some(ch) => ch,
                    None => return Err(format!("Invalid key string: {}", apple_key)),
                };
                if c.is_ascii_lowercase() {
                    // This is a simplified mapping - in reality you'd need a full key code table
                    return Ok(format!("tell application \"System Events\" to keystroke \"{}\"", apple_key));
                }
            }
            return Err(format!("Unsupported key for AppleScript: {}", apple_key));
        }
    };

    script.push_str(key_code);

    // Add modifiers if present
    if !normalized_modifiers.is_empty() {
        script.push_str(" using {");
        let mut apple_modifiers = Vec::new();
        for modifier in &normalized_modifiers {
            match modifier.as_str() {
                "cmd" => apple_modifiers.push("command down"),
                "shift" => apple_modifiers.push("shift down"),
                "option" => apple_modifiers.push("option down"),
                "control" => apple_modifiers.push("control down"),
                _ => return Err(format!("Unsupported modifier for AppleScript: {}", modifier)),
            }
        }
        script.push_str(&apple_modifiers.join(", "));
        script.push('}');
    }

    debug!("Generated AppleScript: {}", script);
    Ok(script)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_key() {
        let parsed = parse_key_combination("a").expect("Should parse simple key 'a'");
        assert_eq!(parsed.key, "a");
        assert_eq!(parsed.modifiers.len(), 0);
    }

    #[test]
    fn test_parse_single_modifier() {
        let parsed = parse_key_combination("cmd+a").expect("Should parse 'cmd+a'");
        assert_eq!(parsed.key, "a");
        assert_eq!(parsed.modifiers, vec!["cmd"]);
    }

    #[test]
    fn test_parse_multiple_modifiers() {
        let parsed = parse_key_combination("cmd+shift+a").expect("Should parse 'cmd+shift+a'");
        assert_eq!(parsed.key, "a");
        assert_eq!(parsed.modifiers, vec!["cmd", "shift"]);
    }

    #[test]
    fn test_split_key_and_modifier() {
        let (key, modifier) = split_key_and_modifier("cmd+a").expect("Should split 'cmd+a'");
        assert_eq!(key, "a");
        assert_eq!(modifier, Some("cmd".to_string()));
    }

    #[test]
    fn test_normalize_modifier() {
        assert_eq!(normalize_modifier("command"), Some("cmd"));
        assert_eq!(normalize_modifier("alt"), Some("option"));
        assert_eq!(normalize_modifier("ctrl"), Some("control"));
        assert_eq!(normalize_modifier("invalid"), None);
    }
}
