#!/usr/bin/env node

/**
 * TypeScript Type Generator for Rust Structs
 * 
 * This script generates TypeScript types from Rust state structures
 * to ensure type safety between the frontend and backend.
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Rust to TypeScript type mappings
const TYPE_MAPPINGS = {
  // Basic types
  'String': 'string',
  'str': 'string',
  'bool': 'boolean',
  'i32': 'number',
  'i64': 'number',
  'u32': 'number',
  'u64': 'number',
  'f32': 'number',
  'f64': 'number',
  'usize': 'number',
  
  // Collections
  'Vec<String>': 'string[]',
  'HashMap<String, String>': 'Record<string, string>',
  'HashMap<String, Value>': 'Record<string, any>',
  'HashMap<String, ToolApprovalRequest>': 'Record<string, ToolApprovalRequest>',
  
  // Special types
  'Value': 'any', // serde_json::Value
  'PathBuf': 'string',
  'Duration': 'number',
  
  // Option types are handled separately
};

// Read and parse Rust state files
function parseRustState() {
  const stateFile = path.join(__dirname, '..', 'src-tauri', 'src', 'state.rs');
  const permissionsFile = path.join(__dirname, '..', 'src-tauri', 'src', 'commands', 'permissions.rs');
  
  const stateContent = fs.readFileSync(stateFile, 'utf8');
  const permissionsContent = fs.readFileSync(permissionsFile, 'utf8');
  
  const types = {
    enums: [],
    structs: []
  };
  
  // Parse enums
  const enumRegex = /#\[derive\([^)]*\)\]\s*pub enum (\w+)\s*{([^}]+)}/g;
  let match;
  
  while ((match = enumRegex.exec(stateContent)) !== null) {
    const enumName = match[1];
    const enumBody = match[2];
    
    // Handle enum variants properly
    const variants = [];
    const lines = enumBody.split('\n');
    
    for (const line of lines) {
      const trimmed = line.trim();
      if (trimmed && !trimmed.startsWith('//')) {
        // Match variant name (before comma or comment)
        const variantMatch = trimmed.match(/^\s*(\w+)/);
        if (variantMatch) {
          variants.push(variantMatch[1]);
        }
      }
    }
    
    if (variants.length > 0) {
      types.enums.push({
        name: enumName,
        variants
      });
    }
  }
  
  // Parse structs
  const structRegex = /#\[derive\([^)]*\)\]\s*pub struct (\w+)\s*{([^}]+)}/g;
  const files = [
    { content: stateContent, file: 'state.rs' },
    { content: permissionsContent, file: 'permissions.rs' }
  ];
  
  for (const { content, file } of files) {
    const regex = new RegExp(structRegex);
    while ((match = regex.exec(content)) !== null) {
      const structName = match[1];
      const structBody = match[2];
      
      const fields = [];
      const fieldRegex = /pub\s+(\w+):\s+(.+?)(?:,|$)/g;
      let fieldMatch;
      
      while ((fieldMatch = fieldRegex.exec(structBody)) !== null) {
        const fieldName = fieldMatch[1];
        let fieldType = fieldMatch[2].trim();
        
        // Remove trailing comma if present
        fieldType = fieldType.replace(/,$/, '').trim();
        
        fields.push({
          name: fieldName,
          type: fieldType,
          optional: fieldType.startsWith('Option<')
        });
      }
      
      types.structs.push({
        name: structName,
        fields,
        source: file
      });
    }
  }
  
  return types;
}

// Convert Rust type to TypeScript type
function convertRustTypeToTS(rustType) {
  // Handle Option types
  if (rustType.startsWith('Option<') && rustType.endsWith('>')) {
    const innerType = rustType.slice(7, -1);
    const tsType = convertRustTypeToTS(innerType);
    return `${tsType} | null`;
  }
  
  // Handle Arc, Mutex, etc wrappers
  const wrappers = ['Arc<', 'Mutex<', 'StdMutex<', 'TokioMutex<', 'RwLock<'];
  for (const wrapper of wrappers) {
    if (rustType.startsWith(wrapper) && rustType.endsWith('>')) {
      const innerType = rustType.slice(wrapper.length, -1);
      return convertRustTypeToTS(innerType);
    }
  }
  
  // Direct mapping
  if (TYPE_MAPPINGS[rustType]) {
    return TYPE_MAPPINGS[rustType];
  }
  
  // Handle generic Vec
  if (rustType.startsWith('Vec<') && rustType.endsWith('>')) {
    const innerType = rustType.slice(4, -1);
    const tsType = convertRustTypeToTS(innerType);
    return `${tsType}[]`;
  }
  
  // Handle HashMap
  if (rustType.startsWith('HashMap<') && rustType.endsWith('>')) {
    const inner = rustType.slice(8, -1);
    const parts = inner.split(',').map(p => p.trim());
    if (parts.length === 2) {
      const keyType = convertRustTypeToTS(parts[0]);
      const valueType = convertRustTypeToTS(parts[1]);
      return `Record<${keyType}, ${valueType}>`;
    }
  }
  
  // If no mapping found, use the type as-is (likely a custom type)
  return rustType;
}

// Generate TypeScript interfaces
function generateTypeScript(types) {
  let output = `/**
 * Auto-generated TypeScript types from Rust state structures
 * Generated on: ${new Date().toISOString()}
 * 
 * DO NOT EDIT MANUALLY - This file is auto-generated
 * Run: npm run generate-types
 */

`;

  // Generate enums
  for (const enumDef of types.enums) {
    output += `export enum ${enumDef.name} {\n`;
    for (const variant of enumDef.variants) {
      output += `  ${variant} = "${variant}",\n`;
    }
    output += `}\n\n`;
  }
  
  // Generate interfaces
  for (const structDef of types.structs) {
    output += `export interface ${structDef.name} {\n`;
    for (const field of structDef.fields) {
      const tsType = convertRustTypeToTS(field.type);
      output += `  ${field.name}: ${tsType};\n`;
    }
    output += `}\n\n`;
  }
  
  // Add simplified frontend state interface
  output += `/**
 * Simplified AppState interface for frontend use
 * This excludes internal implementation details and async locks
 */
export interface FrontendAppState {
  // Audio Settings
  audioSettings: {
    ttsProvider: string;
    dictationActive: boolean;
    dictationClipboardEnabled: boolean;
    soundEnabled: boolean;
    alwaysListeningActive: boolean;
    alwaysListeningSensitivity: number;
    alwaysListeningWakeWords: string[];
    notificationSoundEnabled: boolean;
  };
  
  // Agent Execution State
  agentExecution: {
    executionActive: boolean;
    executionId: string | null;
    currentStep: number | null;
    maxSteps: number | null;
    toolApprovalRequired: boolean;
  };
  
  // UI Settings
  uiSettings: {
    barUiState: string;
    performanceMonitoringEnabled: boolean;
    debugMode: boolean;
    notificationType: string;
    notificationDuration: number;
    notificationPosition: string;
    notificationShowIcons: boolean;
    notificationPersistImportant: boolean;
    smoothMouseMovement: boolean;
  };
  
  // Input Settings
  inputSettings: {
    keyboardShortcuts: KeyboardShortcuts;
    agentTriggerMode: AgentTriggerMode;
    dictationTriggerMode: DictationTriggerMode;
  };
  
  // Permissions State
  permissionsState: PermissionsState | null;
  permissionsChecked: boolean;
  
  // Cloud State
  cloudEnabled: boolean;
  
  // Pending Tool Approvals
  pendingToolApprovals: ToolApprovalRequest[];
}

`;

  // Add type guards
  output += `// Type Guards
export function isPermissionsState(value: any): value is PermissionsState {
  return value &&
    typeof value === 'object' &&
    'accessibility' in value &&
    'screen_recording' in value &&
    'microphone' in value &&
    'input_monitoring' in value &&
    'all_granted' in value;
}

export function isToolApprovalRequest(value: any): value is ToolApprovalRequest {
  return value &&
    typeof value === 'object' &&
    typeof value.tool_id === 'string' &&
    typeof value.tool_name === 'string' &&
    typeof value.description === 'string' &&
    typeof value.timestamp === 'number';
}

export function isKeyboardShortcuts(value: any): value is KeyboardShortcuts {
  return value &&
    typeof value === 'object' &&
    typeof value.agent_mode_toggle === 'string' &&
    typeof value.dictation_input === 'string' &&
    typeof value.stop_current_task === 'string' &&
    typeof value.open_settings === 'string';
}
`;

  return output;
}

// Main execution
function main() {
  console.log('🔧 Parsing Rust state structures...');
  const types = parseRustState();
  
  console.log(`📊 Found ${types.enums.length} enums and ${types.structs.length} structs`);
  
  console.log('🏗️ Generating TypeScript types...');
  const tsOutput = generateTypeScript(types);
  
  const outputPath = path.join(__dirname, '..', 'src', 'types', 'state.ts');
  const outputDir = path.dirname(outputPath);
  
  if (!fs.existsSync(outputDir)) {
    fs.mkdirSync(outputDir, { recursive: true });
  }
  
  fs.writeFileSync(outputPath, tsOutput, 'utf8');
  console.log(`✅ Generated TypeScript types at: ${outputPath}`);
  
  // Log type mappings that might need attention
  const unmappedTypes = new Set();
  for (const structDef of types.structs) {
    for (const field of structDef.fields) {
      const tsType = convertRustTypeToTS(field.type);
      if (tsType === field.type && !field.type.includes('<')) {
        unmappedTypes.add(field.type);
      }
    }
  }
  
  if (unmappedTypes.size > 0) {
    console.log('\n⚠️  The following Rust types might need manual mapping:');
    for (const type of unmappedTypes) {
      console.log(`   - ${type}`);
    }
  }
}

// Run the script
main();