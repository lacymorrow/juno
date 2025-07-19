#!/usr/bin/env node
/**
 * Migration script to replace hardcoded strings with constants
 * This script will:
 * 1. Find all hardcoded event names and command names
 * 2. Replace them with imports from constants.generated.ts
 * 3. Add necessary imports
 */

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const PROJECT_ROOT = path.resolve(__dirname, '..');
const CONSTANTS_PATH = 'src/lib/constants.generated.ts';

// Load the generated constants
const constantsContent = fs.readFileSync(path.join(PROJECT_ROOT, CONSTANTS_PATH), 'utf8');

// Extract event constants
const eventMatches = constantsContent.match(/EVENTS = \{([^}]+)\}/s);
const eventConstants = {};
if (eventMatches) {
    const lines = eventMatches[1].trim().split('\n');
    lines.forEach(line => {
        const match = line.match(/\s*(\w+):\s*['"]([^'"]+)['"]/);
        if (match) {
            eventConstants[match[2]] = match[1];
        }
    });
}

// Extract command constants
const commandMatches = constantsContent.match(/COMMANDS = \{([^}]+)\}/s);
const commandConstants = {};
if (commandMatches) {
    const lines = commandMatches[1].trim().split('\n');
    lines.forEach(line => {
        const match = line.match(/\s*(\w+):\s*['"]([^'"]+)['"]/);
        if (match) {
            commandConstants[match[2]] = match[1];
        }
    });
}

console.log(`Found ${Object.keys(eventConstants).length} event constants`);
console.log(`Found ${Object.keys(commandConstants).length} command constants`);

// Files to migrate
const filesToMigrate = [
    'src/hooks/useBackendEvents.ts',
    'src/hooks/useSettings.ts',
    'src/lib/ui-api.ts',
    'src/App.tsx',
    'src/FloatingPanel.tsx',
    'src/components/AgentExecutionProgressIndicator.tsx',
    'src/components/ChatMessage.tsx',
    'src/components/CommandOverlay.tsx',
    'src/components/FloatingBar.tsx',
    'src/components/ModelSelector.tsx',
    'src/components/ProviderSelector.tsx',
    'src/components/VoiceStatusIndicator.tsx',
    'src/components/onboarding/Onboarding.tsx',
    'src/hooks/useMenuEvents.ts',
];

function migrateFile(filePath) {
    const fullPath = path.join(PROJECT_ROOT, filePath);
    
    if (!fs.existsSync(fullPath)) {
        console.warn(`File not found: ${filePath}`);
        return;
    }
    
    let content = fs.readFileSync(fullPath, 'utf8');
    let modified = false;
    const replacements = [];
    
    // Track which constants we need to import
    const neededEventConstants = new Set();
    const neededCommandConstants = new Set();
    
    // Replace hardcoded event names in listen() calls
    const listenRegex = /listen\s*\(\s*["']([^"']+)["']/g;
    content = content.replace(listenRegex, (match, eventName) => {
        if (eventConstants[eventName]) {
            neededEventConstants.add(eventConstants[eventName]);
            replacements.push(`Event: "${eventName}" → EVENTS.${eventConstants[eventName]}`);
            modified = true;
            return `listen(EVENTS.${eventConstants[eventName]}`;
        }
        return match;
    });
    
    // Replace hardcoded event names in switch cases and comparisons
    const caseRegex = /case\s+["']([^"']+)["']/g;
    content = content.replace(caseRegex, (match, eventName) => {
        if (eventConstants[eventName]) {
            neededEventConstants.add(eventConstants[eventName]);
            replacements.push(`Case: "${eventName}" → EVENTS.${eventConstants[eventName]}`);
            modified = true;
            return `case EVENTS.${eventConstants[eventName]}`;
        }
        return match;
    });
    
    // Replace hardcoded command names in invoke() calls
    const invokeRegex = /invoke\s*(?:<[^>]+>)?\s*\(\s*["']([^"']+)["']/g;
    content = content.replace(invokeRegex, (match, commandName) => {
        if (commandConstants[commandName]) {
            neededCommandConstants.add(commandConstants[commandName]);
            replacements.push(`Command: "${commandName}" → COMMANDS.${commandConstants[commandName]}`);
            modified = true;
            const genericMatch = match.match(/invoke\s*(<[^>]+>)/);
            if (genericMatch) {
                return `invoke${genericMatch[1]}(COMMANDS.${commandConstants[commandName]}`;
            }
            return `invoke(COMMANDS.${commandConstants[commandName]}`;
        }
        return match;
    });
    
    // Add imports if needed
    if (modified && (neededEventConstants.size > 0 || neededCommandConstants.size > 0)) {
        // Check if constants are already imported
        const hasEventsImport = content.includes('EVENTS') && content.includes('constants.generated');
        const hasCommandsImport = content.includes('COMMANDS') && content.includes('constants.generated');
        
        let importStatement = '';
        const imports = [];
        
        if (neededEventConstants.size > 0 && !hasEventsImport) {
            imports.push('EVENTS');
        }
        if (neededCommandConstants.size > 0 && !hasCommandsImport) {
            imports.push('COMMANDS');
        }
        
        if (imports.length > 0) {
            // Calculate relative path from file to constants
            const fileDir = path.dirname(fullPath);
            const constantsFullPath = path.join(PROJECT_ROOT, CONSTANTS_PATH);
            let relativePath = path.relative(fileDir, constantsFullPath).replace(/\\/g, '/');
            if (!relativePath.startsWith('.')) {
                relativePath = './' + relativePath;
            }
            relativePath = relativePath.replace('.ts', '');
            
            importStatement = `import { ${imports.join(', ')} } from '${relativePath}';\n`;
            
            // Add import after other imports
            const importRegex = /^((?:import[^;]+;\s*\n)*)/m;
            const importMatch = content.match(importRegex);
            
            if (importMatch) {
                content = content.replace(importRegex, importMatch[0] + importStatement);
            } else {
                // Add at the beginning if no imports found
                content = importStatement + content;
            }
        }
    }
    
    if (modified) {
        fs.writeFileSync(fullPath, content);
        console.log(`\n✅ Migrated ${filePath}`);
        replacements.forEach(r => console.log(`   - ${r}`));
    } else {
        console.log(`\n⏭️  No changes needed for ${filePath}`);
    }
}

// Run migration
console.log('\n🚀 Starting migration of hardcoded constants...\n');

filesToMigrate.forEach(file => {
    try {
        migrateFile(file);
    } catch (error) {
        console.error(`❌ Error migrating ${file}:`, error.message);
    }
});

console.log('\n✨ Migration complete!');
console.log('\n📋 Next steps:');
console.log('1. Review the changes using git diff');
console.log('2. Run the TypeScript compiler to check for any issues');
console.log('3. Test the application to ensure everything works correctly');
console.log('4. Consider adding an ESLint rule to prevent future hardcoded strings');