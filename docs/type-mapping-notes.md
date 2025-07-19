# TypeScript Type Generation Notes

## Overview
This document outlines the TypeScript type generation from Rust structs and any manual type mappings that need attention.

## Generated Files
- `/src/types/state.ts` - Auto-generated TypeScript interfaces from Rust state structures
- `/src/types/state-validation.ts` - Validation utilities and type guards
- `/src/hooks/useAppStateSync.ts` - React hook for syncing state with Rust backend
- `/scripts/generate-ts-types.js` - Type generation script

## Running Type Generation
```bash
npm run generate-types
```

This command is automatically run during the build process via the `prebuild` script.

## Type Mappings

### Automatic Mappings
The following Rust types are automatically converted:

| Rust Type | TypeScript Type |
|-----------|----------------|
| `String` | `string` |
| `bool` | `boolean` |
| `i32`, `i64`, `u32`, `u64`, `f32`, `f64` | `number` |
| `Vec<T>` | `T[]` |
| `Option<T>` | `T \| null` |
| `HashMap<K, V>` | `Record<K, V>` |
| `PathBuf` | `string` |
| `Duration` | `number` |
| `Value` (serde_json) | `any` |

### Wrapper Types
The following Rust wrapper types are automatically unwrapped:
- `Arc<T>` → `T`
- `Mutex<T>` → `T`
- `StdMutex<T>` → `T`
- `TokioMutex<T>` → `T`
- `RwLock<T>` → `T`

### Manual Type Considerations

1. **Complex Nested Types**: Some deeply nested generic types may need manual review
2. **Custom Structs**: Referenced structs that aren't in the parsed files need to be added
3. **Trait Objects**: Rust trait objects (`dyn Trait`) are mapped to `any` and may need refinement

## Frontend State Interface

The `FrontendAppState` interface provides a simplified view of the Rust `AppState` that:
- Excludes internal implementation details (Arc, Mutex wrappers)
- Flattens nested structures for easier access
- Uses camelCase for TypeScript conventions while maintaining snake_case for Rust compatibility

## Type Safety Best Practices

1. **Always use type guards**: Use the provided validation functions when receiving data from Rust
2. **Avoid `any` types**: Replace `any` with specific types where possible
3. **Use the hook**: The `useAppStateSync` hook provides type-safe state management
4. **Validate at boundaries**: Always validate data coming from IPC calls

## Known Limitations

1. **Enum Comments**: Rust enum variant comments are not preserved
2. **Default Values**: Rust default trait implementations are not captured
3. **Generic Constraints**: Complex generic constraints may need manual adjustment

## Future Improvements

1. **ts-rs Integration**: Consider using the `ts-rs` crate for automatic type generation
2. **Schema Validation**: Add runtime schema validation using Zod
3. **Code Generation**: Generate Tauri command wrappers automatically
4. **Type Tests**: Add type-level tests to ensure compatibility