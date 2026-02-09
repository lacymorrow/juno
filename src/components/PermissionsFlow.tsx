import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  AlertCircle,
  CheckCircle,
  ExternalLink,
  Info,
  Keyboard,
  Lock,
  Mic,
  Monitor,
  RefreshCw,
  Settings,
  Shield,
  Zap,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import {
  getPermissionsStatus,
  invalidatePermissionsCache,
  type PermissionsState,
  type AppPermissionStatus,
} from "@/lib/permissions-service";
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
import { Separator } from "./ui/separator";

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
  autoRedirectEnabled = false,
}: PermissionsFlowProps) {
  const [permissions, setPermissions] = useState<PermissionsState | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isRequestingPermission, setIsRequestingPermission] = useState<
    string | null
  >(null);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);
  const pendingTimers = useRef<ReturnType<typeof setTimeout>[]>([]);

  // Track mounted state for async callbacks
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      for (const timer of pendingTimers.current) {
        clearTimeout(timer);
      }
      pendingTimers.current = [];
    };
  }, []);

  // Check permissions status with optional auto-redirect
  const checkPermissions = async (forceRefresh = false) => {
    try {
      setIsLoading(true);
      setError(null);

      // Use centralized permission service - prevents duplicate calls
      const result = await getPermissionsStatus(forceRefresh);

      setPermissions(result);

      // Auto-complete if all permissions are granted
      if (result.all_granted && onComplete) {
        pendingTimers.current.push(setTimeout(() => {
          if (mountedRef.current) onComplete();
        }, 1000));
      }
    } catch (err) {
      setError(err as string);
      console.error("Error checking permissions:", err);
    } finally {
      setIsLoading(false);
    }
  };

  // Request accessibility permission using native APIs - no password prompts
  const requestAccessibilityPermission = async () => {
    try {
      setIsRequestingPermission("accessibility");
      const granted = await invoke<boolean>(
        "request_accessibility_permission_native"
      );

      // Invalidate cache and force refresh after permission request
      invalidatePermissionsCache();

      if (granted) {
        // Permission was already granted
        await checkPermissions(true);
      } else {
        // System Settings should be open for user to grant permission
        // Wait a moment and then refresh to check if user granted it
        pendingTimers.current.push(setTimeout(async () => {
          if (mountedRef.current) await checkPermissions(true);
        }, 2000));
      }
    } catch (err) {
      setError(err as string);
      console.error("Error requesting accessibility permission:", err);
    } finally {
      setIsRequestingPermission(null);
    }
  };

  // Request screen recording permission using native APIs - no password prompts
  const requestScreenRecordingPermission = async () => {
    try {
      setIsRequestingPermission("screen_recording");
      const granted = await invoke<boolean>(
        "request_screen_recording_permission_native"
      );

      // Invalidate cache and force refresh after permission request
      invalidatePermissionsCache();

      if (granted) {
        // Permission was already granted
        await checkPermissions(true);
      } else {
        // System Settings should be open for user to grant permission
        // Wait a moment and then refresh to check if user granted it
        pendingTimers.current.push(setTimeout(async () => {
          if (mountedRef.current) await checkPermissions(true);
        }, 2000));
      }
    } catch (err) {
      setError(err as string);
      console.error("Error requesting screen recording permission:", err);
    } finally {
      setIsRequestingPermission(null);
    }
  };

  // Request microphone permission using native APIs - no password prompts
  const requestMicrophonePermission = async () => {
    try {
      setIsRequestingPermission("microphone");
      const granted = await invoke<boolean>(
        "request_microphone_permission_native"
      );

      // Invalidate cache and force refresh after permission request
      invalidatePermissionsCache();

      if (granted) {
        // Permission was already granted
        await checkPermissions(true);
      } else {
        // System Settings should be open for user to grant permission
        // Wait a moment and then refresh to check if user granted it
        pendingTimers.current.push(setTimeout(async () => {
          if (mountedRef.current) await checkPermissions(true);
        }, 2000));
      }
    } catch (err) {
      setError(err as string);
      console.error("Error requesting microphone permission:", err);
    } finally {
      setIsRequestingPermission(null);
    }
  };

  // Request input monitoring permission using native APIs - no password prompts
  const requestInputMonitoringPermission = async () => {
    try {
      setIsRequestingPermission("input_monitoring");
      const granted = await invoke<boolean>(
        "request_input_monitoring_permission_native"
      );

      // Invalidate cache and force refresh after permission request
      invalidatePermissionsCache();

      if (granted) {
        // Permission was already granted
        await checkPermissions(true);
      } else {
        // System Settings should be open for user to grant permission
        // Wait a moment and then refresh to check if user granted it
        pendingTimers.current.push(setTimeout(async () => {
          if (mountedRef.current) await checkPermissions(true);
        }, 2000));
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
        permission_type: preferencePane,
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
    console.log("⚠️ [DEBUG] Starting handleSkip - stopping monitoring...");
    await stopMonitoring();
    console.log("⚠️ [DEBUG] Monitoring stopped, calling onSkip...");
    if (onSkip) {
      onSkip();
    }
  };

  // Set up permissions monitoring
  useEffect(() => {
    let mounted = true;
    let unlistenPermissions: (() => void) | undefined;

    const setupListeners = async () => {
      // Listen for permissions changes
      const fn = await listen<PermissionsState>(
        "permissions-changed",
        (event) => {
          if (!mounted || !mountedRef.current) return;
          setPermissions(event.payload);

          // Auto-complete if all permissions are granted
          if (event.payload.all_granted && onComplete) {
            pendingTimers.current.push(setTimeout(() => {
              if (mountedRef.current) onComplete();
            }, 1000));
          }
        }
      );

      if (mounted) {
        unlistenPermissions = fn;
      } else {
        fn(); // Unmounted during setup, clean up immediately
        return;
      }

      // Start monitoring
      await startMonitoring();

      // Initial check with auto-redirect if enabled
      if (mounted) {
        await checkPermissions(autoRedirectEnabled);
      }
    };

    setupListeners();

    return () => {
      mounted = false;
      // Cleanup: stop monitoring and remove event listener
      stopMonitoring().catch((err) => {
        console.error(
          "Error stopping permissions monitoring during cleanup:",
          err
        );
      });
      unlistenPermissions?.();
    };
  }, [onComplete, autoRedirectEnabled]);

  const getPermissionIcon = (permission: AppPermissionStatus) => {
    if (permission.granted) {
      return <CheckCircle className="h-5 w-5 text-green-500" />;
    } else if (permission.required) {
      return <AlertCircle className="h-5 w-5 text-red-500" />;
    } else {
      return <AlertCircle className="h-5 w-5 text-yellow-500" />;
    }
  };

  const getPermissionBadge = (permission: AppPermissionStatus) => {
    if (permission.granted) {
      return (
        <Badge
          variant="outline"
          className="text-green-600 border-green-200 bg-green-50"
        >
          ✓ Granted
        </Badge>
      );
    } else if (permission.required) {
      return (
        <Badge
          variant="destructive"
          className="bg-red-100 text-red-700 border-red-300"
        >
          ⚠ Required
        </Badge>
      );
    } else {
      return (
        <Badge
          variant="secondary"
          className="bg-yellow-100 text-yellow-700 border-yellow-300"
        >
          💡 Optional
        </Badge>
      );
    }
  };

  const getPermissionPriorityIcon = (permission: AppPermissionStatus) => {
    switch (permission.permission_type) {
      case "accessibility":
        return <Lock className="h-5 w-5 text-blue-600" />;
      case "screen_recording":
        return <Monitor className="h-5 w-5 text-purple-600" />;
      case "microphone":
        return <Mic className="h-5 w-5 text-green-600" />;
      case "input_monitoring":
        return <Keyboard className="h-5 w-5 text-orange-600" />;
      default:
        return <Shield className="h-5 w-5 text-gray-600" />;
    }
  };

  const renderPermissionCard = (
    permission: AppPermissionStatus,
    onRequest?: () => void,
    onRequestEnhanced?: () => void
  ) => {
    const isRequired = permission.required;
    const cardClassName = permission.granted
      ? "transition-colors border-green-200 bg-green-50/30"
      : isRequired
      ? "transition-colors border-red-200 bg-red-50/30"
      : "transition-colors border-yellow-200 bg-yellow-50/30";

    return (
      <Card key={permission.permission_type} className={cardClassName}>
        <CardHeader className="pb-4">
          <div className="flex items-start justify-between">
            <div className="flex items-start space-x-3">
              <div className="flex items-center space-x-2">
                {getPermissionPriorityIcon(permission)}
                {getPermissionIcon(permission)}
              </div>
              <div className="flex-1">
                <div className="flex items-center space-x-2 mb-1">
                  <CardTitle className="text-lg">
                    {permission.permission_type
                      .replace("_", " ")
                      .replace(/\b\w/g, (l) => l.toUpperCase())}{" "}
                    Access
                  </CardTitle>
                  {getPermissionBadge(permission)}
                </div>
                <CardDescription className="text-sm">
                  {permission.description}
                </CardDescription>
                {isRequired && !permission.granted && (
                  <div className="flex items-center space-x-1 mt-2">
                    <AlertCircle className="h-4 w-4 text-red-500" />
                    <span className="text-sm font-medium text-red-700">
                      This permission is required for Juno to function properly
                    </span>
                  </div>
                )}
                {!isRequired && !permission.granted && (
                  <div className="flex items-center space-x-1 mt-2">
                    <Info className="h-4 w-4 text-yellow-600" />
                    <span className="text-sm text-yellow-700">
                      Optional - enhances functionality when granted
                    </span>
                  </div>
                )}
              </div>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          {!permission.granted && (
            <div className="space-y-3">
              <div
                className={`p-3 rounded-md border ${
                  isRequired
                    ? "bg-red-50 border-red-200"
                    : "bg-yellow-50 border-yellow-200"
                }`}
              >
                <p
                  className={`text-sm ${
                    isRequired ? "text-red-800" : "text-yellow-800"
                  }`}
                >
                  <strong>
                    {isRequired ? "Action Required:" : "Optional Setup:"}
                  </strong>{" "}
                  {permission.instructions}
                </p>
              </div>
              <div className="flex flex-wrap gap-2">
                {/* Enhanced auto-redirect button for accessibility */}
                {onRequestEnhanced &&
                  autoRedirectEnabled &&
                  permission.permission_type === "accessibility" && (
                    <Button
                      onClick={onRequestEnhanced}
                      disabled={
                        isRequestingPermission === permission.permission_type
                      }
                      size="sm"
                      className="bg-blue-600 hover:bg-blue-700"
                    >
                      {isRequestingPermission === permission.permission_type ? (
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
                      isRequestingPermission === permission.permission_type
                    }
                    size="sm"
                    variant={
                      autoRedirectEnabled &&
                      permission.permission_type === "accessibility"
                        ? "outline"
                        : isRequired
                        ? "default"
                        : "secondary"
                    }
                  >
                    {isRequestingPermission === permission.permission_type ? (
                      <>
                        <RefreshCw className="h-4 w-4 mr-2 animate-spin" />
                        Requesting...
                      </>
                    ) : (
                      <>
                        {isRequired
                          ? "Grant Required Permission"
                          : "Grant Optional Permission"}
                      </>
                    )}
                  </Button>
                )}

                {/* Manual settings button */}
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() =>
                    autoRedirectEnabled
                      ? openSystemPreferencesEnhanced(permission.permission_type)
                      : openSystemPreferences(permission.permission_type)
                  }
                >
                  <Settings className="h-4 w-4 mr-2" />
                  Open System Settings
                  <ExternalLink className="h-4 w-4 ml-2" />
                </Button>
              </div>

              {/* Auto-redirect feature notice */}
              {autoRedirectEnabled &&
                permission.permission_type === "accessibility" && (
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
            <div className="flex items-center space-x-2 p-3 bg-green-50 rounded-md border border-green-200">
              <CheckCircle className="h-4 w-4 text-green-600" />
              <p className="text-sm text-green-800 font-medium">
                Permission granted -{" "}
                {permission.permission_type.replace("_", " ")} access is working
                properly
              </p>
            </div>
          )}
        </CardContent>
      </Card>
    );
  };

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
            {permissions.all_granted ? (
              <CheckCircle className="h-5 w-5 text-green-500" />
            ) : (
              <Shield className="h-5 w-5 text-blue-500" />
            )}
            <span>macOS Permissions Setup for {permissions.app_name}</span>
          </CardTitle>
          <CardDescription>
            {permissions.all_granted
              ? "✅ All permissions configured! Juno is ready for full functionality."
              : autoRedirectEnabled
              ? "Configure the permissions below to enable Juno's AI computer use capabilities. Required permissions are marked with ⚠️."
              : "Configure the permissions below to enable Juno's AI computer use capabilities. Some permissions are required for core functionality."}
          </CardDescription>
        </CardHeader>
        {!permissions.all_granted && (
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

      {/* Required Permissions Section */}
      <div className="space-y-4">
        <div className="flex items-center space-x-2">
          <AlertCircle className="h-5 w-5 text-red-500" />
          <h3 className="text-lg font-semibold text-red-800">
            Required Permissions
          </h3>
          <Badge variant="destructive" className="bg-red-100 text-red-700">
            2 Required
          </Badge>
        </div>
        <p className="text-sm text-gray-600 mb-4">
          These permissions are essential for Juno's core AI computer use
          functionality. Without them, Juno cannot automate tasks or interact
          with your desktop.
        </p>

        <div className="space-y-3">
          {/* Accessibility Permission */}
          {permissions.accessibility.required &&
            renderPermissionCard(
              permissions.accessibility,
              requestAccessibilityPermission
            )}

          {/* Screen Recording Permission */}
          {permissions.screen_recording.required &&
            renderPermissionCard(
              permissions.screen_recording,
              requestScreenRecordingPermission
            )}
        </div>
      </div>

      {/* Optional Permissions Section */}
      <Separator />
      <div className="space-y-4">
        <div className="flex items-center space-x-2">
          <Info className="h-5 w-5 text-yellow-600" />
          <h3 className="text-lg font-semibold text-yellow-800">
            Optional Permissions
          </h3>
          <Badge variant="secondary" className="bg-yellow-100 text-yellow-700">
            Enhances Experience
          </Badge>
        </div>
        <p className="text-sm text-gray-600 mb-4">
          These permissions enhance Juno's functionality but are not required
          for basic operation. You can grant them now or skip and enable them
          later.
        </p>

        <div className="space-y-3">
          {/* Microphone Permission */}
          {!permissions.microphone.required &&
            renderPermissionCard(
              permissions.microphone,
              requestMicrophonePermission
            )}

          {/* Input Monitoring Permission */}
          {!permissions.input_monitoring.required &&
            renderPermissionCard(
              permissions.input_monitoring,
              requestInputMonitoringPermission
            )}
        </div>
      </div>

      {/* Success Footer */}
      {permissions.all_granted && (
        <Card className="border-green-200 bg-green-50">
          <CardContent className="pt-6">
            <div className="flex items-center justify-between">
              <div className="flex items-center space-x-3">
                <CheckCircle className="h-6 w-6 text-green-600" />
                <div>
                  <span className="text-green-800 font-semibold text-lg">
                    🎉 Setup Complete!
                  </span>
                  <p className="text-green-700 text-sm mt-1">
                    All permissions configured. Juno is ready for AI computer
                    use.
                  </p>
                </div>
              </div>
              {onComplete && (
                <Button
                  onClick={onComplete}
                  className="bg-green-600 hover:bg-green-700"
                >
                  Continue to Juno
                </Button>
              )}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Skip Option */}
      {showSkipOption && !permissions.all_granted && onSkip && (
        <div className="text-center">
          <Button variant="ghost" onClick={handleSkip} size="sm">
            Skip setup for now (limited functionality)
          </Button>
          <p className="text-xs text-gray-500 mt-1">
            You can complete setup later in Settings
          </p>
        </div>
      )}
    </div>
  );
}
