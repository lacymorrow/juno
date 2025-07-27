#!/usr/bin/env python3
"""
Fix the interaction.rs file to properly use pooled event sources and fix error handling
"""

import re

# Read the file
with open('/Users/lacymorrow/repo/juno/src-tauri/mcp-server-os-level/src/platforms/macos/interaction.rs', 'r') as f:
    content = f.read()

# Fix the error patterns where 'e' is used but not defined
# Pattern 1: AutomationError::PlatformError(format!("...: {}", e))
content = re.sub(
    r'AutomationError::PlatformError\(format!\("([^"]+): \{\}", e\)\)',
    r'AutomationError::PlatformError("\1".to_string())',
    content
)

# Pattern 2: .map_err(|_| AutomationError::PlatformError(format!("...: {}", e)))?
content = re.sub(
    r'\.map_err\(\|_\| AutomationError::PlatformError\(format!\("([^"]+): \{\}", e\)\)\)',
    r'.map_err(|_| AutomationError::PlatformError("\1".to_string()))',
    content
)

# Fix CGEventSource::new patterns that weren't replaced properly
content = re.sub(
    r'CGEventSource::new\(CGEventSourceStateID::HIDSystemState\)\.map_err\(\|_\|\s*{\s*AutomationError::PlatformError\("([^"]+)"\.to_string\(\)\)\s*}\)',
    r'get_pooled_event_source().map_err(|e| AutomationError::PlatformError(format!("\1: {}", e)))',
    content
)

# Fix remaining CGEventSource::new calls
content = re.sub(
    r'CGEventSource::new\(CGEventSourceStateID::HIDSystemState\)',
    r'get_pooled_event_source()',
    content
)

# Write the fixed content back
with open('/Users/lacymorrow/repo/juno/src-tauri/mcp-server-os-level/src/platforms/macos/interaction.rs', 'w') as f:
    f.write(content)

print("Fixed interaction.rs file")