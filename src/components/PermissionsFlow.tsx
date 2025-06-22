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
import { Separator } from "./ui/separator";

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
  autoRedirectEnabled = false,
}: PermissionsFlowProps) {
  const [permissions, setPermissions] = useState<PermissionsState | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isRequestingPermission, setIsRequestingPermission] = useState<
    string | null
  >(null);
  const [error, setError] = useState<string | null>(null);

  // Check permissions status with optional auto-redirect
  const checkPermissions = async (_useAutoRedirect = false) => {
    try {
      setIsLoading(true);
      setError(null);

      // Use native permission checking - eliminates all password prompts
      const result = await invoke<PermissionsState>(
        "check_permissions_status_native"
      );

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

  // Request accessibility permission using native APIs - no password prompts
  const requestAccessibilityPermission = async () => {
    try {
      setIsRequestingPermission("accessibility");
      const granted = await invoke<boolean>(
        "request_accessibility_permission_native"
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
      return <AlertCircle className="h-5 w-5 text-yellow-500" />;
    }
  };

  const getPermissionBadge = (permission: PermissionStatus) => {
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

  const getPermissionPriorityIcon = (permission: PermissionStatus) => {
    switch (permission.permissionType) {
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
    permission: PermissionStatus,
    onRequest?: () => void,
    onRequestEnhanced?: () => void
  ) => {
    const isRequesting = isRequestingPermission === permission.permissionType;
    const gradientClass = permission.granted
      ? "from-green-50/50 to-emerald-50/30 dark:from-green-950/30 dark:to-emerald-950/20 border-green-200/50 dark:border-green-800/50"
      : permission.required
        ? "from-red-50/50 to-rose-50/30 dark:from-red-950/30 dark:to-rose-950/20 border-red-200/50 dark:border-red-800/50"
        : "from-amber-50/50 to-orange-50/30 dark:from-amber-950/30 dark:to-orange-950/20 border-amber-200/50 dark:border-amber-800/50";

    return (
      <Card key={permission.permissionType} className={`rounded-xl bg-gradient-to-r ${gradientClass} backdrop-blur-sm border shadow-sm hover:shadow-md transition-all duration-200`}>
        <CardHeader className="pb-3">
          <div className="flex items-start justify-between">
            <div className="flex items-center gap-3">
              {getPermissionIcon(permission)}
              <div className="space-y-1">
                <CardTitle className="text-base font-semibold text-foreground">
                  {permission.permissionType === "accessibility" && "Accessibility"}
                  {permission.permissionType === "screen_recording" && "Screen Recording"}
                  {permission.permissionType === "microphone" && "Microphone"}
                  {permission.permissionType === "input_monitoring" && "Input Monitoring"}
                </CardTitle>
                <div className="flex items-center gap-2">
                  {getPermissionBadge(permission)}
                  {getPermissionPriorityIcon(permission)}
                </div>
              </div>
            </div>
          </div>
        </CardHeader>
        <CardContent className="pt-0 space-y-4">
          <CardDescription className="text-sm text-muted-foreground leading-relaxed">
            {permission.description}
          </CardDescription>

          {!permission.granted && (
            <div className="space-y-3">
              <div className="p-3 rounded-lg bg-background/50 backdrop-blur-sm border border-border/30">
                <p className="text-xs text-muted-foreground font-medium mb-1">Instructions:</p>
                <p className="text-xs text-muted-foreground leading-relaxed">
                  {permission.instructions}
                </p>
              </div>

              <div className="flex gap-2">
                {onRequest && (
                  <Button
                    onClick={onRequest}
                    disabled={isRequesting}
                    size="sm"
                    className="flex-1 bg-gradient-to-r from-blue-600 to-indigo-600 hover:from-blue-700 hover:to-indigo-700 text-white shadow-sm hover:shadow-md transition-all duration-200"
                  >
                    {isRequesting ? (
                      <>
                        <RefreshCw className="w-3 h-3 mr-2 animate-spin" />
                        Requesting...
                      </>
                    ) : (
                      <>
                        <Shield className="w-3 h-3 mr-2" />
                        Grant Permission
                      </>
                    )}
                  </Button>
                )}

                {onRequestEnhanced && (
                  <Button
                    onClick={onRequestEnhanced}
                    variant="outline"
                    size="sm"
                    className="flex-1 border-border/50 hover:bg-muted/50 transition-all duration-200"
                  >
                    <Settings className="w-3 h-3 mr-2" />
                    Open Settings
                  </Button>
                )}
              </div>
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
        <Alert variant="destructive" className="rounded-xl border-red-200/50 bg-gradient-to-r from-red-50/50 to-rose-50/30 dark:from-red-950/30 dark:to-rose-950/20">
          <AlertCircle className="h-4 w-4" />
          <AlertDescription className="text-red-800 dark:text-red-200">
            Error checking permissions: {error}
          </AlertDescription>
        </Alert>
        <div className="flex gap-3">
          <Button onClick={() => checkPermissions()} className="flex-1 bg-gradient-to-r from-blue-600 to-indigo-600 hover:from-blue-700 hover:to-indigo-700 text-white rounded-xl shadow-lg hover:shadow-xl transition-all duration-200">
            <RefreshCw className="h-4 w-4 mr-2" />
            Try Again
          </Button>
          {autoRedirectEnabled && (
            <Button
              onClick={() => checkPermissions(true)}
              variant="outline"
              className="flex-1 border-border/50 hover:bg-muted/50 rounded-xl transition-all duration-200"
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
    <div className={`space-y-8 ${className}`}>
      {/* Header */}
      <Card className="rounded-2xl bg-gradient-to-r from-blue-50/50 to-indigo-50/30 dark:from-blue-950/30 dark:to-indigo-950/20 border border-blue-200/50 dark:border-blue-800/50 backdrop-blur-sm shadow-lg">
        <CardHeader className="pb-4">
          <CardTitle className="flex items-center gap-3 text-xl">
            {permissions.allGranted ? (
              <CheckCircle className="h-6 w-6 text-green-500" />
            ) : (
              <Shield className="h-6 w-6 text-blue-500" />
            )}
            <span className="bg-gradient-to-r from-blue-700 to-indigo-700 dark:from-blue-300 dark:to-indigo-300 bg-clip-text text-transparent font-semibold">
              macOS Permissions Setup for {permissions.appName}
            </span>
          </CardTitle>
          <CardDescription className="text-muted-foreground leading-relaxed">
            {permissions.allGranted
              ? "✅ All permissions configured! Juno is ready for full functionality."
              : "Configure the permissions below to enable Juno's AI computer use capabilities. Required permissions are marked with priority indicators."}
          </CardDescription>
        </CardHeader>
        {!permissions.allGranted && (
          <CardContent>
            <div className="flex gap-3">
              <Button
                onClick={() => checkPermissions(true)}
                size="sm"
                className="bg-gradient-to-r from-green-600 to-emerald-600 hover:from-green-700 hover:to-emerald-700 text-white rounded-xl shadow-sm hover:shadow-md transition-all duration-200"
              >
                <RefreshCw className="h-4 w-4 mr-2" />
                Refresh Status
              </Button>
              {autoRedirectEnabled && (
                <Button
                  onClick={() => checkPermissions(true)}
                  size="sm"
                  variant="outline"
                  className="border-border/50 hover:bg-muted/50 rounded-xl transition-all duration-200"
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
      <div className="space-y-5">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-full bg-red-100 dark:bg-red-900/30">
            <AlertCircle className="h-5 w-5 text-red-600 dark:text-red-400" />
          </div>
          <div className="flex-1">
            <h3 className="text-lg font-semibold text-red-800 dark:text-red-200">
              Required Permissions
            </h3>
            <p className="text-sm text-muted-foreground">
              Essential for Juno's core AI computer use functionality
            </p>
          </div>
          <Badge variant="destructive" className="bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 border-red-200 dark:border-red-800 rounded-full px-3 py-1">
            2 Required
          </Badge>
        </div>

        <div className="grid gap-4">
          {/* Accessibility Permission */}
          {permissions.accessibility.required &&
            renderPermissionCard(
              permissions.accessibility,
              requestAccessibilityPermission
            )}

          {/* Screen Recording Permission */}
          {permissions.screenRecording.required &&
            renderPermissionCard(
              permissions.screenRecording,
              requestScreenRecordingPermission
            )}
        </div>
      </div>

      {/* Optional Permissions Section */}
      <Separator className="bg-border/30" />
      <div className="space-y-5">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-full bg-amber-100 dark:bg-amber-900/30">
            <Info className="h-5 w-5 text-amber-600 dark:text-amber-400" />
          </div>
          <div className="flex-1">
            <h3 className="text-lg font-semibold text-amber-800 dark:text-amber-200">
              Optional Permissions
            </h3>
            <p className="text-sm text-muted-foreground">
              Enhance functionality but not required for basic operation
            </p>
          </div>
          <Badge variant="secondary" className="bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300 border-amber-200 dark:border-amber-800 rounded-full px-3 py-1">
            Enhances Experience
          </Badge>
        </div>

        <div className="grid gap-4">
          {/* Microphone Permission */}
          {!permissions.microphone.required &&
            renderPermissionCard(
              permissions.microphone,
              requestMicrophonePermission
            )}

          {/* Input Monitoring Permission */}
          {!permissions.inputMonitoring.required &&
            renderPermissionCard(
              permissions.inputMonitoring,
              requestInputMonitoringPermission
            )}
        </div>
      </div>

      {/* Success Footer */}
      {permissions.allGranted && (
        <Card className="rounded-2xl bg-gradient-to-r from-green-50/50 to-emerald-50/30 dark:from-green-950/30 dark:to-emerald-950/20 border border-green-200/50 dark:border-green-800/50 backdrop-blur-sm shadow-lg">
          <CardContent className="pt-6">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-4">
                <div className="p-3 rounded-full bg-green-100 dark:bg-green-900/30">
                  <CheckCircle className="h-8 w-8 text-green-600 dark:text-green-400" />
                </div>
                <div>
                  <span className="text-green-800 dark:text-green-200 font-semibold text-xl">
                    🎉 Setup Complete!
                  </span>
                  <p className="text-green-700 dark:text-green-300 text-sm mt-1">
                    All permissions configured. Juno is ready for AI computer use.
                  </p>
                </div>
              </div>
              {onComplete && (
                <Button
                  onClick={onComplete}
                  className="bg-gradient-to-r from-green-600 to-emerald-600 hover:from-green-700 hover:to-emerald-700 text-white rounded-xl shadow-lg hover:shadow-xl transition-all duration-200 px-6 py-3"
                >
                  Continue to Juno
                </Button>
              )}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Skip Option */}
      {showSkipOption && !permissions.allGranted && onSkip && (
        <div className="text-center space-y-2">
          <Button variant="ghost" onClick={handleSkip} size="sm" className="text-muted-foreground hover:text-foreground hover:bg-muted/50 rounded-xl transition-all duration-200">
            Skip setup for now (limited functionality)
          </Button>
          <p className="text-xs text-muted-foreground">
            You can complete setup later in Settings
          </p>
        </div>
      )}
    </div>
  );
}
