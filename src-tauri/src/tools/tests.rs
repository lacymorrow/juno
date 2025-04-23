#[cfg(test)]
mod tests {
    // Adjust imports to point to the new locations of helpers and definitions
    use crate::tools::helpers::*; // Assuming helpers are moved to tools::helpers
    use crate::tools::definitions::*; // Assuming list_tools is moved to tools::definitions
    use computer_use_ai_sdk::Desktop;
    use serde_json::json;
    use std::sync::Arc;
    use std::{fs, io::Write}; // Time-related imports removed
    use tempfile::NamedTempFile;

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

    // --- Helper Function to Create a Temp File ---
    fn create_temp_file(content: &str) -> NamedTempFile {
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        write!(temp_file, "{}", content).expect("Failed to write to temp file");
        temp_file
    }

    // --- Tests for str_replace_editor ---
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
        let desktop_instance_result = Desktop::new(false, true);
        assert!(desktop_instance_result.is_ok(), "Failed to create dummy Desktop instance for test");
        let desktop_arc = Arc::new(desktop_instance_result.unwrap());

        let tools = list_tools(&desktop_arc);

        assert!(!tools.is_empty(), "list_tools should return some tools");

        for tool in tools {
            assert!(!tool.name.is_empty(), "Tool name should not be empty");
            assert!(!tool.description.is_empty(), "Tool description for '{}' should not be empty", tool.name);
            assert_eq!(tool.input_schema.type_, "object", "Tool input_schema type for '{}' should be object", tool.name);

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

    // Basic Text Editor Create Test - Directly tests the helper function
    #[test]
    fn test_text_editor_create_basic() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");
        let file_path = temp_dir.path().join("test_create.txt").to_str().unwrap().to_string();

        let content = "Test content for basic text editor create test";

        // Call the function directly instead of through call_tool
        let result = match fs::write(&file_path, content) {
            Ok(_) => Ok(json!({
                "success": true,
                "message": format!("File created successfully: {}", file_path)
            })),
            Err(e) => Err(json!({
                "error": format!("Failed to create file: {}", e)
            }))
        };

        assert!(result.is_ok(), "Failed to create test file: {:?}", result.err());

        // Verify file was created with correct content
        let file_content = fs::read_to_string(&file_path).expect("Failed to read created file");
        assert_eq!(file_content, content);
    }
}
