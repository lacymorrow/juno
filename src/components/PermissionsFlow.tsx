import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  AlertCircle,
  CheckCircle,
  ExternalLink,
  RefreshCw,
  Settings,
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
  allGranted: boolean;
  appName: string;
}

interface PermissionsFlowProps {
  onComplete?: () => void;
  onSkip?: () => void;
  showSkipOption?: boolean;
  className?: string;
}

export function PermissionsFlow({
  onComplete,
  onSkip,
  showSkipOption = false,
  className = "",
}: PermissionsFlowProps) {
  const [permissions, setPermissions] = useState<PermissionsState | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isRequestingPermission, setIsRequestingPermission] = useState<
    string | null
  >(null);
  const [error, setError] = useState<string | null>(null);

  // Check permissions status
  const checkPermissions = async () => {
    try {
      setIsLoading(true);
      setError(null);
      const result = await invoke<PermissionsState>("check_permissions_status");
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

  // Request accessibility permission with system prompt
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

  // Open System Preferences
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

      // Initial check
      await checkPermissions();
    };

    setupListeners();

    return () => {
      unlistenPermissions?.();
    };
  }, [onComplete]);

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
    onRequest?: () => void
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
            <div className="flex space-x-2">
              {onRequest && (
                <Button
                  onClick={onRequest}
                  disabled={
                    isRequestingPermission === permission.permissionType
                  }
                  size="sm"
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
              <Button
                variant="outline"
                size="sm"
                onClick={() => openSystemPreferences(permission.permissionType)}
              >
                <Settings className="h-4 w-4 mr-2" />
                Open Settings
                <ExternalLink className="h-4 w-4 ml-2" />
              </Button>
            </div>
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
              Verifying macOS permissions for Juno...
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
        <Button onClick={checkPermissions} className="w-full">
          <RefreshCw className="h-4 w-4 mr-2" />
          Try Again
        </Button>
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
            <Settings className="h-6 w-6" />
            <span>Permissions Setup</span>
          </CardTitle>
          <CardDescription>
            {permissions.appName} needs certain macOS permissions to provide AI
            computer use features.
            {permissions.allGranted
              ? " All permissions are configured!"
              : " Please grant the required permissions below."}
          </CardDescription>
        </CardHeader>
      </Card>

      {/* All permissions granted success */}
      {permissions.allGranted && (
        <Alert>
          <CheckCircle className="h-4 w-4" />
          <AlertDescription>
            🎉 All permissions granted! {permissions.appName} is ready to use.
          </AlertDescription>
        </Alert>
      )}

      {/* Permission Cards */}
      <div className="space-y-4">
        {renderPermissionCard(
          permissions.accessibility,
          requestAccessibilityPermission
        )}
        {renderPermissionCard(permissions.screenRecording)}
        {renderPermissionCard(permissions.microphone)}
      </div>

      {/* Action Buttons */}
      <div className="flex justify-between space-x-3">
        <Button
          variant="outline"
          onClick={checkPermissions}
          disabled={isLoading}
        >
          <RefreshCw className="h-4 w-4 mr-2" />
          Refresh Status
        </Button>

        <div className="flex space-x-3">
          {showSkipOption && onSkip && (
            <Button variant="outline" onClick={onSkip}>
              Skip for Now
            </Button>
          )}

          {permissions.allGranted && onComplete && (
            <Button onClick={onComplete}>Continue</Button>
          )}
        </div>
      </div>

      {/* Help Text */}
      <Alert>
        <AlertCircle className="h-4 w-4" />
        <AlertDescription>
          <strong>Having trouble?</strong> After granting permissions in System
          Preferences, you may need to restart {permissions.appName} for changes
          to take effect.
        </AlertDescription>
      </Alert>
    </div>
  );
}
