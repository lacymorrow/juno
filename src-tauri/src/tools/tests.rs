#[cfg(test)]
mod tests {
    // Adjust imports to point to the new locations of helpers and definitions
    use crate::tools::helpers::*; // Assuming helpers are moved to tools::helpers
    use crate::tools::definitions::*; // Assuming list_tools is moved to tools::definitions
    use computer_use_ai_sdk::Desktop;
    use serde_json::json;
    use std::sync::Arc;

    // --- Tests for get_string_param ---
    #[test]
    fn test_get_string_param_success() {
        let input = json!({ "key": "value" });
        assert_eq!(get_string_param(&input, "key").unwrap(), "value");
    }

    #[test]
    fn test_get_string_param_missing() {
        let input = json!({ "other_key": "value" });
        assert!(get_string_param(&input, "key").is_err());
        // Optionally check the error message if desired
        // assert_eq!(get_string_param(&input, "key").unwrap_err()["error"], json!("Missing or invalid string parameter: key"));
    }

    #[test]
    fn test_get_string_param_wrong_type() {
        let input = json!({ "key": 123 });
        assert!(get_string_param(&input, "key").is_err());
    }

    #[test]
    fn test_get_string_param_null() {
        let input = json!({ "key": null });
        assert!(get_string_param(&input, "key").is_err());
    }

    // --- Tests for get_optional_string_param ---
    #[test]
    fn test_get_optional_string_param_success() {
        let input = json!({ "key": "value" });
        assert_eq!(get_optional_string_param(&input, "key").unwrap(), Some("value".to_string()));
    }

    #[test]
    fn test_get_optional_string_param_missing() {
        let input = json!({ "other_key": "value" });
        assert_eq!(get_optional_string_param(&input, "key").unwrap(), None);
    }

    #[test]
    fn test_get_optional_string_param_null() {
        let input = json!({ "key": null });
        assert_eq!(get_optional_string_param(&input, "key").unwrap(), None);
    }

    #[test]
    fn test_get_optional_string_param_wrong_type() {
        let input = json!({ "key": 123 });
        assert!(get_optional_string_param(&input, "key").is_err());
    }

    // --- Tests for get_f64_param ---
    #[test]
    fn test_get_f64_param_success() {
        let input = json!({ "key": 123.45 });
        assert_eq!(get_f64_param(&input, "key").unwrap(), 123.45);
    }
     #[test]
    fn test_get_f64_param_integer() {
        let input = json!({ "key": 123 }); // Should cast integer to f64
        assert_eq!(get_f64_param(&input, "key").unwrap(), 123.0);
    }


    #[test]
    fn test_get_f64_param_missing() {
        let input = json!({});
        assert!(get_f64_param(&input, "key").is_err());
    }

    #[test]
    fn test_get_f64_param_wrong_type() {
        let input = json!({ "key": "not a number" });
        assert!(get_f64_param(&input, "key").is_err());
    }

    #[test]
    fn test_get_f64_param_null() {
        let input = json!({ "key": null });
        assert!(get_f64_param(&input, "key").is_err());
    }

     // --- Tests for get_u64_param ---
    #[test]
    fn test_get_u64_param_success() {
        let input = json!({ "key": 123 });
        assert_eq!(get_u64_param(&input, "key").unwrap(), 123);
    }

    #[test]
    fn test_get_u64_param_missing() {
        let input = json!({});
        assert!(get_u64_param(&input, "key").is_err());
    }

    #[test]
    fn test_get_u64_param_wrong_type_string() {
        let input = json!({ "key": "123" });
        assert!(get_u64_param(&input, "key").is_err());
    }

    #[test]
    fn test_get_u64_param_wrong_type_float() {
        let input = json!({ "key": 123.45 });
        assert!(get_u64_param(&input, "key").is_err());
    }

     #[test]
    fn test_get_u64_param_wrong_type_negative() {
        let input = json!({ "key": -123 });
        assert!(get_u64_param(&input, "key").is_err());
    }

    #[test]
    fn test_get_u64_param_null() {
        let input = json!({ "key": null });
        assert!(get_u64_param(&input, "key").is_err());
    }

    // --- Tests for get_i64_param ---
     #[test]
    fn test_get_i64_param_success_positive() {
        let input = json!({ "key": 123 });
        assert_eq!(get_i64_param(&input, "key").unwrap(), 123);
    }

     #[test]
    fn test_get_i64_param_success_negative() {
        let input = json!({ "key": -123 });
        assert_eq!(get_i64_param(&input, "key").unwrap(), -123);
    }

    #[test]
    fn test_get_i64_param_missing() {
        let input = json!({});
        assert!(get_i64_param(&input, "key").is_err());
    }

    #[test]
    fn test_get_i64_param_wrong_type_string() {
        let input = json!({ "key": "123" });
        assert!(get_i64_param(&input, "key").is_err());
    }

     #[test]
    fn test_get_i64_param_wrong_type_float() {
        let input = json!({ "key": 123.45 });
        assert!(get_i64_param(&input, "key").is_err());
    }

    #[test]
    fn test_get_i64_param_null() {
        let input = json!({ "key": null });
        assert!(get_i64_param(&input, "key").is_err());
    }

    // --- Tests for get_optional_u64_param ---
    #[test]
    fn test_get_optional_u64_param_success() {
        let input = json!({ "key": 123 });
        assert_eq!(get_optional_u64_param(&input, "key").unwrap(), Some(123));
    }

    #[test]
    fn test_get_optional_u64_param_missing() {
        let input = json!({});
        assert_eq!(get_optional_u64_param(&input, "key").unwrap(), None);
    }

    #[test]
    fn test_get_optional_u64_param_null() {
        let input = json!({ "key": null });
        assert_eq!(get_optional_u64_param(&input, "key").unwrap(), None);
    }

    #[test]
    fn test_get_optional_u64_param_wrong_type_string() {
        let input = json!({ "key": "123" });
        assert!(get_optional_u64_param(&input, "key").is_err());
    }

     #[test]
    fn test_get_optional_u64_param_wrong_type_negative() {
        let input = json!({ "key": -123 });
        assert!(get_optional_u64_param(&input, "key").is_err());
    }

    // --- Tests for get_optional_bool_param ---
     #[test]
    fn test_get_optional_bool_param_success_true() {
        let input = json!({ "key": true });
        assert_eq!(get_optional_bool_param(&input, "key").unwrap(), Some(true));
    }

     #[test]
    fn test_get_optional_bool_param_success_false() {
        let input = json!({ "key": false });
        assert_eq!(get_optional_bool_param(&input, "key").unwrap(), Some(false));
    }

    #[test]
    fn test_get_optional_bool_param_missing() {
        let input = json!({});
        assert_eq!(get_optional_bool_param(&input, "key").unwrap(), None);
    }

    #[test]
    fn test_get_optional_bool_param_null() {
        let input = json!({ "key": null });
        assert_eq!(get_optional_bool_param(&input, "key").unwrap(), None);
    }

    #[test]
    fn test_get_optional_bool_param_wrong_type_string() {
        let input = json!({ "key": "true" });
        assert!(get_optional_bool_param(&input, "key").is_err());
    }

     #[test]
    fn test_get_optional_bool_param_wrong_type_number() {
        let input = json!({ "key": 1 });
        assert!(get_optional_bool_param(&input, "key").is_err());
    }

     // --- Tests for get_optional_modifier_keys ---
    #[test]
    fn test_get_optional_modifier_keys_success() {
        let input = json!({ "modifier_keys": ["cmd", "shift"] });
        assert_eq!(
            get_optional_modifier_keys(&input).unwrap(),
            Some(vec!["cmd".to_string(), "shift".to_string()])
        );
    }

    #[test]
    fn test_get_optional_modifier_keys_empty_array() {
        let input = json!({ "modifier_keys": [] });
        assert_eq!(
            get_optional_modifier_keys(&input).unwrap(),
            Some(Vec::<String>::new())
        );
    }

    #[test]
    fn test_get_optional_modifier_keys_missing() {
        let input = json!({});
        assert_eq!(get_optional_modifier_keys(&input).unwrap(), None);
    }

    #[test]
    fn test_get_optional_modifier_keys_null() {
        let input = json!({ "modifier_keys": null });
        assert_eq!(get_optional_modifier_keys(&input).unwrap(), None);
    }

    #[test]
    fn test_get_optional_modifier_keys_wrong_type_string() {
        let input = json!({ "modifier_keys": "cmd" });
        assert!(get_optional_modifier_keys(&input).is_err());
    }

    #[test]
    fn test_get_optional_modifier_keys_wrong_type_in_array() {
        let input = json!({ "modifier_keys": ["cmd", 123] });
        assert!(get_optional_modifier_keys(&input).is_err());
    }

    // --- Test for list_tools ---
    #[test]
    fn test_list_tools_basic() {
        // Need a Desktop instance, even if not used by list_tools currently.
        // Create a dummy one (assuming Desktop::new is accessible and works for testing)
        // If Desktop::new is complex or requires unavailable resources, mocking is needed.
        // For now, let's assume a simple creation works for this test.
        // Note: The false, true args might need adjustment based on Desktop::new's meaning
        let desktop_instance_result = Desktop::new(false, true);
        assert!(desktop_instance_result.is_ok(), "Failed to create dummy Desktop instance for test");
        let desktop_instance = desktop_instance_result.unwrap();
        let desktop_arc = Arc::new(desktop_instance);

        let tools = list_tools(&desktop_arc);

        assert!(!tools.is_empty(), "list_tools should return some tools");

        // Check for the presence of a few key tools by name
        let tool_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
        assert!(tool_names.contains(&"type_text".to_string()), "Missing tool: type_text");
        assert!(tool_names.contains(&"click_focused_element".to_string()), "Missing tool: click_focused_element");
        assert!(tool_names.contains(&"bash".to_string()), "Missing tool: bash");
        assert!(tool_names.contains(&"text_editor_view".to_string()), "Missing tool: text_editor_view");

        // Optional: Check properties of a specific tool (e.g., type_text)
        if let Some(type_text_tool) = tools.iter().find(|t| t.name == "type_text") {
            assert_eq!(type_text_tool.input_schema.type_, "object");
            assert!(type_text_tool.input_schema.properties.contains_key("text"));
            assert_eq!(type_text_tool.input_schema.required, vec!["text".to_string()]);
        } else {
            panic!("type_text tool definition not found for detailed check");
        }
    }
}
