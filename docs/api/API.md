# API Reference (Canonical)

This is the canonical location for the API reference. For a minimal entry point, see `../SIMPLE_DOCS.md`.

---

## Breaking Changes

### PR #383: Permissions API Field Naming Convention Change

**Date**: December 2024

The `NativePermissionStatus` struct and related permission types now use **snake_case** field names instead of camelCase.

#### Changed Fields

| Before (camelCase) | After (snake_case) |
|-------------------|-------------------|
| `permissionType` | `permission_type` |
| `screenRecording` | `screen_recording` |
| `inputMonitoring` | `input_monitoring` |
| `allGranted` | `all_granted` |
| `appName` | `app_name` |

#### Migration Steps

1. Update frontend type definitions in `src/types/settings.ts` to use snake_case
2. Update all component references from camelCase to snake_case (e.g., `permissions.screenRecording` → `permissions.screen_recording`)
3. Update invoke parameter names (e.g., `permissionType` → `permission_type`)

#### Affected Commands

- `check_permissions_status_native` - Response payload uses snake_case
- `open_system_settings_enhanced` - Parameter `permission_type` (was `permissionType`)

---

# API Reference

(Full content will be consolidated from the root `API.md` in this phase.)


