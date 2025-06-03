# Juno AI Computer Use Agent - Cursor Rules

This directory contains consolidated cursor rules for the Juno AI Computer Use Agent project. The rules have been organized into a logical hierarchy to eliminate redundancy and provide clear guidance for development.

## Rule Structure

### Core Rules (00-04)
These are the primary rules that cover the main aspects of the project:

#### [00-project-overview.mdc](00-project-overview.mdc)
- **Purpose**: High-level project description and architecture overview
- **Contents**: Technology stack, project structure, key features, development workflow
- **Use When**: Getting oriented with the project or explaining it to others

#### [01-voice-system.mdc](01-voice-system.mdc)
- **Purpose**: Comprehensive voice control and transcription system guide
- **Contents**: Voice plugin architecture, audio processing, event flow, debugging
- **Use When**: Working on voice transcription, audio processing, or dictation features

#### [02-ai-agent-system.mdc](02-ai-agent-system.mdc)
- **Purpose**: AI agent architecture and execution flow
- **Contents**: Agent components, execution flow, tool integration, state management
- **Use When**: Developing AI agent features, tool integration, or agent behavior

#### [03-computer-use-tools.mdc](03-computer-use-tools.mdc)
- **Purpose**: Computer use tools and implementation details
- **Contents**: macOS implementation, browser automation, tool development, agent tools guide
- **Use When**: Implementing computer automation features, platform-specific code, or tool development

#### [04-tauri-development.mdc](04-tauri-development.mdc)
- **Purpose**: Tauri development guidelines and best practices
- **Contents**: Plugin development, Rust workflow, frontend integration, error handling
- **Use When**: Developing Tauri plugins, Rust backend code, or frontend-backend integration

### Specialized Guides (05-06)
These rules cover specific technical areas and advanced patterns:

#### [05-specialized-guides.mdc](05-specialized-guides.mdc)
- **Purpose**: Technical guides for specific features and components
- **Contents**: Screenshot handling, system prompts, audio diagnostics, bar state sync, CLI tools
- **Use When**: Working on specific technical challenges or implementing specialized features

#### [06-advanced-voice-integration.mdc](06-advanced-voice-integration.mdc)
- **Purpose**: Advanced voice integration patterns and plugin architecture
- **Contents**: Plugin integration, event flow, state synchronization, AI agent interaction
- **Use When**: Implementing complex voice features, plugin development, or advanced integration patterns

## Rule Usage Guidelines

### For New Developers
1. Start with **00-project-overview.mdc** to understand the project
2. Read **04-tauri-development.mdc** for development workflow
3. Focus on specific rules based on your work area (voice, AI, tools)

### For Feature Development
1. Check the relevant core rule (01-04) for your feature area
2. Consult specialized guides (05-06) for specific technical challenges
3. Follow the development workflow in the Tauri development guide

### For Debugging
1. Use **01-voice-system.mdc** for voice/audio issues
2. Use **02-ai-agent-system.mdc** for agent execution problems
3. Use **05-specialized-guides.mdc** for specific component issues

## Consolidation Benefits

### Eliminated Redundancy
- Removed 25+ overlapping rule files
- Consolidated similar content into logical groupings
- Eliminated empty or minimal files

### Improved Organization
- Numbered files for logical reading order
- Clear separation between core and specialized content
- Comprehensive coverage without duplication

### Enhanced Maintainability
- Fewer files to maintain and update
- Clear ownership of content areas
- Easier to find relevant information

## Development Workflow Integration

These rules integrate with the project's development workflow:

1. **Cargo Check Requirement**: Always run `cargo check --manifest-path src-tauri/Cargo.toml` after Rust changes
2. **File-by-File Changes**: Make changes incrementally with user review
3. **Testing Strategy**: Use DevToolsPanel for manual testing, follow voice dictation test flow
4. **Error Handling**: Implement robust error handling throughout the system

## Maintenance

When updating these rules:
1. Keep content focused and avoid duplication
2. Update relevant sections when implementing new features
3. Maintain the logical hierarchy and numbering
4. Ensure examples and code snippets remain current

This consolidated rule structure provides comprehensive guidance while maintaining clarity and avoiding redundancy. 
