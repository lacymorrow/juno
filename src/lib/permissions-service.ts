/**
 * Centralized permissions service to prevent duplicate permission check calls.
 * All components should use this service instead of calling invoke directly.
 */
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { PermissionsState } from "@/types/settings";
export type { PermissionsState, AppPermissionStatus } from "@/types/settings";

// Cache configuration
const CACHE_TTL = 5000; // 5 seconds - shorter TTL for better responsiveness when user returns from System Settings

// Cache state
let cachedPermissions: PermissionsState | null = null;
let cacheTimestamp = 0;
let pendingRequest: Promise<PermissionsState> | null = null;
let focusListenerInitialized = false;
let focusUnlisten: (() => void) | null = null;

/**
 * Initialize window focus listener to invalidate cache when app regains focus.
 * This ensures fresh permission status when returning from System Settings.
 */
async function initFocusListener(): Promise<void> {
  if (focusListenerInitialized) return;

  try {
    const currentWindow = getCurrentWindow();
    focusUnlisten = await currentWindow.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        // Invalidate cache when window gains focus
        // This handles the case where user grants permission in System Settings
        invalidatePermissionsCache();
      }
    });
    focusListenerInitialized = true;
  } catch (error) {
    console.warn("Failed to initialize focus listener for permissions:", error);
  }
}

/**
 * Check if the cached permissions are still valid
 */
function isCacheValid(): boolean {
  return cachedPermissions !== null && Date.now() - cacheTimestamp < CACHE_TTL;
}

/**
 * Get permissions status with caching and deduplication.
 * This prevents multiple parallel calls from triggering multiple backend requests.
 */
export async function getPermissionsStatus(
  forceRefresh = false
): Promise<PermissionsState> {
  // Initialize focus listener on first use (handles cache invalidation when returning from System Settings)
  void initFocusListener();

  if (forceRefresh) {
    // Wait for any pending request to complete first to avoid race conditions
    // where multiple requests are in-flight and results could be inconsistent
    if (pendingRequest) {
      await pendingRequest.catch(() => {
        // Ignore errors from pending request - we're about to make a fresh one
      });
    }
    invalidatePermissionsCache();
    return startPermissionsRequest();
  }

  // Return cached value if valid and not forcing refresh
  if (isCacheValid() && cachedPermissions) {
    return cachedPermissions;
  }

  // If a request is already in progress, wait for it instead of starting another
  if (pendingRequest) {
    return pendingRequest;
  }

  // Start a new request
  return startPermissionsRequest();
}

/**
 * Invalidate the permissions cache.
 * Call this after requesting a permission change.
 */
export function invalidatePermissionsCache(): void {
  cachedPermissions = null;
  cacheTimestamp = 0;
}

/**
 * Get cached permissions without making a request.
 * Returns null if cache is empty or expired.
 */
export function getCachedPermissions(): PermissionsState | null {
  if (isCacheValid()) {
    return cachedPermissions;
  }
  return null;
}

/**
 * Reset internal state (test-only).
 */
export function __resetPermissionsServiceCacheForTests(): void {
  cachedPermissions = null;
  cacheTimestamp = 0;
  pendingRequest = null;
  if (focusUnlisten) {
    focusUnlisten();
    focusUnlisten = null;
  }
  focusListenerInitialized = false;
}

function startPermissionsRequest(): Promise<PermissionsState> {
  const request = invoke<PermissionsState>("check_permissions_status_native")
    .then((result) => {
      if (pendingRequest === request) {
        cachedPermissions = result;
        cacheTimestamp = Date.now();
        pendingRequest = null;
      }
      return result;
    })
    .catch((error) => {
      if (pendingRequest === request) {
        pendingRequest = null;
      }
      throw error;
    });

  pendingRequest = request;
  return request;
}
