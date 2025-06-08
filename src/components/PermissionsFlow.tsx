import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  AlertCircle,
  CheckCircle,
  ExternalLink,
  RefreshCw,
  Settings,
  Zap,
} from "lucide-react";
import { useEffect, useState } from "react";
import { Alert, AlertDescription } from "./ui/alert";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "./ui/card";

interface PermissionStatus {
  permissionType: string;
  granted: boolean;
  required: boolean;
  description: string;
  instructions: string;
}

interface PermissionsState {
  accessibility: PermissionStatus;
  screenRecording: PermissionStatus;
  microphone: PermissionStatus;
  inputMonitoring: PermissionStatus;
  allGranted: boolean;
  appName: string;
}

interface PermissionsFlowProps {
  onComplete?: () => void;
  onSkip?: () => void;
  showSkipOption?: boolean;
  className?: string;
  autoRedirectEnabled?: boolean;
}

export function PermissionsFlow({
  onComplete,
  onSkip,
  showSkipOption = false,
  className = "",
  autoRedirectEnabled = true,
}: PermissionsFlowProps) {
  const [permissions, setPermissions] = useState<PermissionsState | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isRequestingPermission, setIsRequestingPermission] = useState<
    string | null
  >(null);
  const [error, setError] = useState<string | null>(null);
  const [skipped, setSkipped] = useState(false);

  // Check permissions status with optional auto-redirect
  const checkPermissions = async (useAutoRedirect = false) => {
    try {
      setIsLoading(true);
      setError(null);

      const result =
        useAutoRedirect && autoRedirectEnabled
          ? await invoke<PermissionsState>(
              "check_permissions_status_with_auto_redirect",
              { autoOpenSettings: true }
            )
          : await invoke<PermissionsState>("check_permissions_status");

      setPermissions(result);

      // Auto-complete if all permissions are granted
      if (result.allGranted && onComplete) {
        setTimeout(() => onComplete(), 1000);
      }
    } catch (err) {
      setError(err as string);
      console.error("Error checking permissions:", err);
    } finally {
      setIsLoading(false);
    }
  };

  // Enhanced accessibility permission request with auto-redirect
  const requestAccessibilityPermissionEnhanced = async (
    withAutoRedirect = true
  ) => {
    try {
      setIsRequestingPermission("accessibility");

      const granted =
        withAutoRedirect && autoRedirectEnabled
          ? await invoke<boolean>(
              "request_accessibility_permission_with_auto_redirect",
              { autoOpenSettings: true }
            )
          : await invoke<boolean>("request_accessibility_permission");

      if (granted) {
        // Refresh permissions status
        await checkPermissions();
      } else if (!withAutoRedirect) {
        // If not using auto-redirect and not granted, offer manual opening
        await openSystemPreferences("accessibility");
      }
      // If using auto-redirect, the system settings should already be open
    } catch (err) {
      setError(err as string);
      console.error("Error requesting accessibility permission:", err);
    } finally {
      setIsRequestingPermission(null);
    }
  };

  // Original accessibility permission request (for backward compatibility)
  const requestAccessibilityPermission = async () => {
    try {
      setIsRequestingPermission("accessibility");
      const granted = await invoke<boolean>("request_accessibility_permission");

      if (granted) {
        // Refresh permissions status
        await checkPermissions();
      } else {
        // If not granted, open System Preferences
        await openSystemPreferences("accessibility");
      }
    } catch (err) {
      setError(err as string);
      console.error("Error requesting accessibility permission:", err);
    } finally {
      setIsRequestingPermission(null);
    }
  };

  // Request screen recording permission with enhanced system settings navigation
  const requestScreenRecordingPermission = async () => {
    try {
      setIsRequestingPermission("screen_recording");
      const granted = await invoke<boolean>(
        "request_screen_recording_permission"
      );

      if (granted) {
        // Refresh permissions status
        await checkPermissions();
      } else {
        // Permission not granted - System Settings should be open automatically
        // Wait a moment and then refresh to see if user granted it
        setTimeout(async () => {
          await checkPermissions();
        }, 2000);
      }
    } catch (err) {
      setError(err as string);
      console.error("Error requesting screen recording permission:", err);
    } finally {
      setIsRequestingPermission(null);
    }
  };

  // Request microphone permission with system dialog trigger and settings navigation
  const requestMicrophonePermission = async () => {
    try {
      setIsRequestingPermission("microphone");
      const granted = await invoke<boolean>("request_microphone_permission");

      if (granted) {
        // Permission was granted immediately
        await checkPermissions();
      } else {
        // Permission dialog was shown or System Settings opened
        // Wait a moment and then refresh to check if user granted it
        setTimeout(async () => {
          await checkPermissions();
        }, 2000);
      }
    } catch (err) {
      setError(err as string);
      console.error("Error requesting microphone permission:", err);
    } finally {
      setIsRequestingPermission(null);
    }
  };

  // Request input monitoring permission with enhanced system settings navigation
  const requestInputMonitoringPermission = async () => {
    try {
      setIsRequestingPermission("input_monitoring");
      const granted = await invoke<boolean>(
        "request_input_monitoring_permission"
      );

      if (granted) {
        // Permission was already granted
        await checkPermissions();
      } else {
        // System Settings should be open for user to grant permission
        // Wait a moment and then refresh to check if user granted it
        setTimeout(async () => {
          await checkPermissions();
        }, 2000);
      }
    } catch (err) {
      setError(err as string);
      console.error("Error requesting input monitoring permission:", err);
    } finally {
      setIsRequestingPermission(null);
    }
  };

  // Enhanced system preferences opening
  const openSystemPreferencesEnhanced = async (preferencePane: string) => {
    try {
      await invoke("open_system_settings_enhanced", {
        permissionType: preferencePane,
      });
    } catch (err) {
      setError(err as string);
      console.error("Error opening enhanced System Settings:", err);
    }
  };

  // Open System Preferences (original method)
  const openSystemPreferences = async (preferencePane: string) => {
    try {
      await invoke("open_system_preferences", { preferencePane });
    } catch (err) {
      setError(err as string);
      console.error("Error opening System Preferences:", err);
    }
  };

  // Start monitoring permissions changes
  const startMonitoring = async () => {
    try {
      await invoke("start_permissions_monitoring");
    } catch (err) {
      console.error("Error starting permissions monitoring:", err);
    }
  };

  // Stop monitoring permissions changes
  const stopMonitoring = async () => {
    try {
      await invoke("stop_permissions_monitoring");
    } catch (err) {
      console.error("Error stopping permissions monitoring:", err);
    }
  };

  // Handle skip with proper cleanup
  const handleSkip = async () => {
    setSkipped(true);
    console.log("⚠️ [DEBUG] Starting handleSkip - stopping monitoring...");
    await stopMonitoring();
    console.log("⚠️ [DEBUG] Monitoring stopped, calling onSkip...");
    if (onSkip) {
      onSkip();
    }
  };

  // Set up permissions monitoring
  useEffect(() => {
    let unlistenPermissions: (() => void) | undefined;

    const setupListeners = async () => {
      // Listen for permissions changes
      unlistenPermissions = await listen<PermissionsState>(
        "permissions-changed",
        (event) => {
          setPermissions(event.payload);

          // Auto-complete if all permissions are granted
          if (event.payload.allGranted && onComplete) {
            setTimeout(() => onComplete(), 1000);
          }
        }
      );

      // Start monitoring
      await startMonitoring();

      // Initial check with auto-redirect if enabled
      await checkPermissions(autoRedirectEnabled);
    };

    setupListeners();

    return () => {
      // Cleanup: stop monitoring and remove event listener
      // Since cleanup function cannot be async, we don't await but still call the async function
      stopMonitoring().catch((err) => {
        console.error(
          "Error stopping permissions monitoring during cleanup:",
          err
        );
      });
      unlistenPermissions?.();
    };
  }, [onComplete, autoRedirectEnabled]);

  const getPermissionIcon = (permission: PermissionStatus) => {
    if (permission.granted) {
      return <CheckCircle className="h-5 w-5 text-green-500" />;
    } else if (permission.required) {
      return <AlertCircle className="h-5 w-5 text-red-500" />;
    } else {
      return <AlertCircle className="h-5 w-5 text-gray-400" />;
    }
  };

  const getPermissionBadge = (permission: PermissionStatus) => {
    if (permission.granted) {
      return (
        <Badge variant="outline" className="text-green-600 border-green-200">
          Granted
        </Badge>
      );
    } else if (permission.required) {
      return <Badge variant="destructive">Required</Badge>;
    } else {
      return <Badge variant="secondary">Optional</Badge>;
    }
  };

  const renderPermissionCard = (
    permission: PermissionStatus,
    onRequest?: () => void,
    onRequestEnhanced?: () => void
  ) => (
    <Card key={permission.permissionType} className="transition-colors">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-2">
            {getPermissionIcon(permission)}
            <CardTitle className="text-lg capitalize">
              {permission.permissionType.replace("_", " ")} Access
            </CardTitle>
          </div>
          {getPermissionBadge(permission)}
        </div>
        <CardDescription>{permission.description}</CardDescription>
      </CardHeader>
      <CardContent>
        {!permission.granted && permission.required && (
          <div className="space-y-3">
            <p className="text-sm text-muted-foreground">
              {permission.instructions}
            </p>
            <div className="flex flex-wrap gap-2">
              {/* Enhanced auto-redirect button for accessibility */}
              {onRequestEnhanced &&
                autoRedirectEnabled &&
                permission.permissionType === "accessibility" && (
                  <Button
                    onClick={onRequestEnhanced}
                    disabled={
                      isRequestingPermission === permission.permissionType
                    }
                    size="sm"
                    className="bg-blue-600 hover:bg-blue-700"
                  >
                    {isRequestingPermission === permission.permissionType ? (
                      <>
                        <RefreshCw className="h-4 w-4 mr-2 animate-spin" />
                        Opening Settings...
                      </>
                    ) : (
                      <>
                        <Zap className="h-4 w-4 mr-2" />
                        Auto-Grant Permission
                      </>
                    )}
                  </Button>
                )}

              {/* Standard request button */}
              {onRequest && (
                <Button
                  onClick={onRequest}
                  disabled={
                    isRequestingPermission === permission.permissionType
                  }
                  size="sm"
                  variant={
                    autoRedirectEnabled &&
                    permission.permissionType === "accessibility"
                      ? "outline"
                      : "default"
                  }
                >
                  {isRequestingPermission === permission.permissionType ? (
                    <>
                      <RefreshCw className="h-4 w-4 mr-2 animate-spin" />
                      Requesting...
                    </>
                  ) : (
                    "Request Permission"
                  )}
                </Button>
              )}

              {/* Manual settings button */}
              <Button
                variant="outline"
                size="sm"
                onClick={() =>
                  autoRedirectEnabled
                    ? openSystemPreferencesEnhanced(permission.permissionType)
                    : openSystemPreferences(permission.permissionType)
                }
              >
                <Settings className="h-4 w-4 mr-2" />
                Open Settings
                <ExternalLink className="h-4 w-4 ml-2" />
              </Button>
            </div>

            {/* Auto-redirect feature notice */}
            {autoRedirectEnabled &&
              permission.permissionType === "accessibility" && (
                <div className="mt-2 p-2 bg-blue-50 rounded-md border border-blue-200">
                  <p className="text-xs text-blue-700">
                    <Zap className="h-3 w-3 inline mr-1" />
                    Auto-redirect enabled: System Settings will open
                    automatically when needed
                  </p>
                </div>
              )}
          </div>
        )}
        {permission.granted && (
          <p className="text-sm text-green-600">
            ✓ Permission granted - {permission.permissionType.replace("_", " ")}{" "}
            access is working
          </p>
        )}
      </CardContent>
    </Card>
  );

  if (isLoading) {
    return (
      <div className={`space-y-6 ${className}`}>
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center space-x-2">
              <RefreshCw className="h-5 w-5 animate-spin" />
              <span>Checking Permissions</span>
            </CardTitle>
            <CardDescription>
              {autoRedirectEnabled
                ? "Verifying macOS permissions for Juno with auto-redirect..."
                : "Verifying macOS permissions for Juno..."}
            </CardDescription>
          </CardHeader>
        </Card>
      </div>
    );
  }

  if (error) {
    return (
      <div className={`space-y-6 ${className}`}>
        <Alert variant="destructive">
          <AlertCircle className="h-4 w-4" />
          <AlertDescription>
            Error checking permissions: {error}
          </AlertDescription>
        </Alert>
        <div className="flex gap-2">
          <Button onClick={() => checkPermissions()} className="flex-1">
            <RefreshCw className="h-4 w-4 mr-2" />
            Try Again
          </Button>
          {autoRedirectEnabled && (
            <Button
              onClick={() => checkPermissions(true)}
              variant="outline"
              className="flex-1"
            >
              <Zap className="h-4 w-4 mr-2" />
              Retry with Auto-Open
            </Button>
          )}
        </div>
      </div>
    );
  }

  if (!permissions) {
    return null;
  }

  return (
    <div className={`space-y-6 ${className}`}>
      {/* Header */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center space-x-2">
            {permissions.allGranted ? (
              <CheckCircle className="h-5 w-5 text-green-500" />
            ) : (
              <AlertCircle className="h-5 w-5 text-red-500" />
            )}
            <span>macOS Permissions for {permissions.appName}</span>
          </CardTitle>
          <CardDescription>
            {permissions.allGranted
              ? "All required permissions are granted. Juno is ready to use!"
              : autoRedirectEnabled
              ? "Some permissions are missing. Use Auto-Grant for the easiest setup, or grant them manually."
              : "Some permissions are missing. Please grant the required permissions to use Juno."}
          </CardDescription>
        </CardHeader>
        {!permissions.allGranted && (
          <CardContent>
            <div className="flex flex-wrap gap-2">
              <Button
                onClick={() => checkPermissions(true)}
                size="sm"
                className="bg-green-600 hover:bg-green-700"
              >
                <RefreshCw className="h-4 w-4 mr-2" />
                Refresh Status
              </Button>
              {autoRedirectEnabled && (
                <Button
                  onClick={() => checkPermissions(true)}
                  size="sm"
                  variant="outline"
                >
                  <Zap className="h-4 w-4 mr-2" />
                  Check with Auto-Open
                </Button>
              )}
            </div>
          </CardContent>
        )}
      </Card>

      {/* Permission Cards */}
      <div className="space-y-4">
        {/* Accessibility Permission */}
        {renderPermissionCard(
          permissions.accessibility,
          requestAccessibilityPermission,
          autoRedirectEnabled
            ? () => requestAccessibilityPermissionEnhanced(true)
            : undefined
        )}

        {/* Screen Recording Permission */}
        {renderPermissionCard(
          permissions.screenRecording,
          requestScreenRecordingPermission
        )}

        {/* Microphone Permission */}
        {renderPermissionCard(
          permissions.microphone,
          requestMicrophonePermission
        )}

        {/* Input Monitoring Permission */}
        {renderPermissionCard(
          permissions.inputMonitoring,
          requestInputMonitoringPermission
        )}
      </div>

      {/* Footer */}
      {permissions.allGranted && (
        <Card className="border-green-200 bg-green-50">
          <CardContent className="pt-6">
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-2">
                <CheckCircle className="h-5 w-5 text-green-600" />
                <span className="text-green-800 font-medium">
                  All permissions granted!
                </span>
              </div>
              {onComplete && (
                <Button
                  onClick={onComplete}
                  className="bg-green-600 hover:bg-green-700"
                >
                  Continue
                </Button>
              )}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Skip Option */}
      {showSkipOption && !permissions.allGranted && onSkip && (
        <div className="text-center">
          <Button variant="ghost" onClick={handleSkip} size="sm">
            Skip for now (some features may not work)
          </Button>
        </div>
      )}
    </div>
  );
}
