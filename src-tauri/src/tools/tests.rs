#[cfg(test)]
mod tests {
    // Adjust imports to point to the new locations of helpers and definitions
    use crate::tools::helpers::*; // Assuming helpers are moved to tools::helpers
    use crate::tools::definitions::*; // Assuming list_tools is moved to tools::definitions
    use computer_use_ai_sdk::Desktop;
    use serde_json::json;
    use std::sync::Arc;
    use std::{fs, io::Write}; // Added for file operations in tests
    use tempfile::NamedTempFile; // Added for temporary file creation

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

    // --- Tests for str_replace_editor ---

    // Helper function to create a temporary file with content
    fn create_temp_file(content: &str) -> NamedTempFile {
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        write!(temp_file, "{}", content).expect("Failed to write to temp file");
        temp_file
    }

    #[test]
    fn test_str_replace_editor_success() {
        let temp_file = create_temp_file("Hello world, this is a test.");
        let file_path = temp_file.path().to_str().unwrap().to_string();

        let result = str_replace_editor(file_path.clone(), "world".to_string(), "universe".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), format!("Successfully updated file \'{}\'", file_path));

        let updated_content = fs::read_to_string(&file_path).expect("Failed to read back temp file");
        assert_eq!(updated_content, "Hello universe, this is a test.");
    }

    #[test]
    fn test_str_replace_editor_find_text_not_found() {
        let temp_file = create_temp_file("Hello world, this is a test.");
        let file_path = temp_file.path().to_str().unwrap().to_string();

        let result = str_replace_editor(file_path.clone(), "galaxy".to_string(), "universe".to_string());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), format!("No replacement performed: \'galaxy\' was not found in file \'{}\'.", file_path));

        // Ensure file content is unchanged
        let content = fs::read_to_string(&file_path).expect("Failed to read back temp file");
        assert_eq!(content, "Hello world, this is a test.");
    }

     #[test]
    fn test_str_replace_editor_find_text_multiple_occurrences() {
        let temp_file = create_temp_file("test line one test\\ntest line two test");
        let file_path = temp_file.path().to_str().unwrap().to_string();

        let result = str_replace_editor(file_path.clone(), "test".to_string(), "verify".to_string());
        assert!(result.is_err());
        // The exact line numbers might vary based on how lines are split, adjust assertion if needed
        assert!(result.unwrap_err().starts_with("No replacement performed: \'test\' found multiple times in file"));

        // Ensure file content is unchanged
        let content = fs::read_to_string(&file_path).expect("Failed to read back temp file");
        assert_eq!(content, "test line one test\\ntest line two test");
    }

    #[test]
    fn test_str_replace_editor_file_not_exist() {
        let non_existent_path = "/path/to/non/existent/file/hopefully";
        let result = str_replace_editor(non_existent_path.to_string(), "find".to_string(), "replace".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().starts_with("Failed to read file"));
    }

    // --- Test for list_tools ---
    #[test]
    fn test_list_tools_basic() {
        // Need a Desktop instance, even if not used by list_tools currently.
        // Create a dummy one (assuming Desktop::new is accessible and works for testing)
        // If Desktop::new is complex or requires unavailable resources, mocking is needed.
        // For now, let's assume a simple creation works for this test.
        let desktop_instance_result = Desktop::new(false, true);
        assert!(desktop_instance_result.is_ok(), "Failed to create dummy Desktop instance for test");
        let desktop_arc = Arc::new(desktop_instance_result.unwrap());

        let tools = list_tools(&desktop_arc);

        assert!(!tools.is_empty(), "list_tools should return some tools");

        for tool in tools {
            assert!(!tool.name.is_empty(), "Tool name should not be empty");
            assert!(!tool.description.is_empty(), "Tool description for '{}' should not be empty", tool.name);
            assert_eq!(tool.input_schema.type_, "object", "Tool input_schema type for '{}' should be object", tool.name);
            // Optionally, further checks on properties/required fields could be added here if needed.
            // Check that properties is a HashMap (implied by type) and required is a Vec<String> (implied by type).

            // Ensure required fields are actually listed in properties
            let props_map = &tool.input_schema.properties;
            for required_prop_str in &tool.input_schema.required {
                assert!(
                    props_map.contains_key(required_prop_str),
                    "Required property '{}' for tool '{}' must be defined in properties",
                    required_prop_str,
                    tool.name
                );
            }
        }

        // Keep a check for a few key tools by name as a sanity check
        let tool_names: Vec<String> = list_tools(&desktop_arc).iter().map(|t| t.name.clone()).collect();
        assert!(tool_names.contains(&"type_text".to_string()), "Missing tool: type_text");
        assert!(tool_names.contains(&"click_focused_element".to_string()), "Missing tool: click_focused_element");
        assert!(tool_names.contains(&"bash".to_string()), "Missing tool: bash");
        assert!(tool_names.contains(&"text_editor_view".to_string()), "Missing tool: text_editor_view");

    }
}
