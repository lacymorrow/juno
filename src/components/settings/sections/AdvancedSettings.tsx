import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { SettingsSectionProps } from "../types";
import {
  AlertCircle,
  CheckCircle,
  MonitorSpeaker,
  RefreshCw,
  Shield,
  Terminal,
  Square,
} from "lucide-react";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

interface PermissionState {
  granted: boolean;
  required: boolean;
}

interface PermissionsState {
  accessibility: PermissionState;
  screenRecording: PermissionState;
  microphone: PermissionState;
  allGranted: boolean;
}

interface AdvancedSettingsProps extends SettingsSectionProps {
  onNavigateToDevTools?: () => void;
  onNavigateToChat?: () => void;
  onNavigateToPermissions?: () => void;
}

export default function AdvancedSettings({
  onNavigateToDevTools,
  onNavigateToChat,
  onNavigateToPermissions
}: AdvancedSettingsProps) {
  const [permissionsState, setPermissionsState] = useState<PermissionsState | null>(null);
  const [permissionsLoading, setPermissionsLoading] = useState(false);

  const loadPermissionsStatus = async () => {
    setPermissionsLoading(true);
    try {
      const permissions = await invoke<PermissionsState>("check_permissions_status");
      setPermissionsState(permissions);
    } catch (error) {
      console.error("Failed to load permissions:", error);
      toast.error("Failed to check permissions status");
    } finally {
      setPermissionsLoading(false);
    }
  };

  const getPermissionIcon = (granted: boolean, required: boolean) => {
    if (!required) return <Square className="h-4 w-4 text-gray-400" />;
    return granted ? (
      <CheckCircle className="h-4 w-4 text-green-500" />
    ) : (
      <AlertCircle className="h-4 w-4 text-red-500" />
    );
  };

  const getPermissionBadge = (granted: boolean, required: boolean) => {
    if (!required) {
      return <Badge variant="outline">Optional</Badge>;
    }
    return (
      <Badge variant={granted ? "default" : "destructive"}>
        {granted ? "Granted" : "Required"}
      </Badge>
    );
  };

  useEffect(() => {
    loadPermissionsStatus();
  }, []);

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">Advanced</h3>

        {/* Developer Tools */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Terminal size={20} />
              Developer Tools
            </CardTitle>
            <CardDescription>
              Access developer tools and application windows
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <Button
                variant="outline"
                className="h-20 flex flex-col items-center justify-center gap-2"
                onClick={onNavigateToDevTools}
              >
                <Terminal size={24} />
                <div className="text-center">
                  <div className="font-medium">Developer Tools</div>
                  <div className="text-xs text-muted-foreground">
                    Debug and testing tools
                  </div>
                </div>
              </Button>

              <Button
                variant="outline"
                className="h-20 flex flex-col items-center justify-center gap-2"
                onClick={onNavigateToChat}
              >
                <MonitorSpeaker size={24} />
                <div className="text-center">
                  <div className="font-medium">Main Chat</div>
                  <div className="text-xs text-muted-foreground">
                    Return to main interface
                  </div>
                </div>
              </Button>
            </div>

            <p className="text-sm text-muted-foreground">
              You can also access Developer Tools from the system tray menu or use
              the toggle button in the main interface.
            </p>
          </CardContent>
        </Card>

        {/* Permissions Settings */}
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Shield size={20} />
              macOS Permissions
            </CardTitle>
            <CardDescription>
              Manage system permissions required for AI computer use features
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {permissionsLoading ? (
              <div className="flex items-center gap-2 text-muted-foreground">
                <RefreshCw className="h-4 w-4 animate-spin" />
                <span>Checking permissions...</span>
              </div>
            ) : permissionsState ? (
              <div className="space-y-3">
                {/* Overall Status */}
                <div className="flex items-center justify-between p-3 bg-muted/50 rounded-lg">
                  <div className="flex items-center gap-2">
                    {permissionsState.allGranted ? (
                      <CheckCircle className="h-5 w-5 text-green-500" />
                    ) : (
                      <AlertCircle className="h-5 w-5 text-red-500" />
                    )}
                    <span className="font-medium">
                      {permissionsState.allGranted
                        ? "All permissions granted"
                        : "Some permissions missing"}
                    </span>
                  </div>
                  <Badge
                    variant={
                      permissionsState.allGranted ? "default" : "destructive"
                    }
                  >
                    {permissionsState.allGranted ? "Ready" : "Needs Setup"}
                  </Badge>
                </div>

                {/* Individual Permissions */}
                <div className="grid gap-2">
                  {/* Accessibility */}
                  <div className="flex items-center justify-between p-2 border rounded">
                    <div className="flex items-center gap-2">
                      {getPermissionIcon(
                        permissionsState.accessibility.granted,
                        permissionsState.accessibility.required
                      )}
                      <span className="text-sm">Accessibility</span>
                    </div>
                    {getPermissionBadge(
                      permissionsState.accessibility.granted,
                      permissionsState.accessibility.required
                    )}
                  </div>

                  {/* Screen Recording */}
                  <div className="flex items-center justify-between p-2 border rounded">
                    <div className="flex items-center gap-2">
                      {getPermissionIcon(
                        permissionsState.screenRecording.granted,
                        permissionsState.screenRecording.required
                      )}
                      <span className="text-sm">Screen Recording</span>
                    </div>
                    {getPermissionBadge(
                      permissionsState.screenRecording.granted,
                      permissionsState.screenRecording.required
                    )}
                  </div>

                  {/* Microphone */}
                  <div className="flex items-center justify-between p-2 border rounded">
                    <div className="flex items-center gap-2">
                      {getPermissionIcon(
                        permissionsState.microphone.granted,
                        permissionsState.microphone.required
                      )}
                      <span className="text-sm">Microphone</span>
                    </div>
                    {getPermissionBadge(
                      permissionsState.microphone.granted,
                      permissionsState.microphone.required
                    )}
                  </div>
                </div>

                {/* Action Buttons */}
                <div className="flex gap-2 pt-2">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={loadPermissionsStatus}
                    disabled={permissionsLoading}
                  >
                    <RefreshCw className="h-4 w-4 mr-1" />
                    Refresh
                  </Button>
                  <Button
                    onClick={onNavigateToPermissions}
                    size="sm"
                    variant={permissionsState.allGranted ? "outline" : "default"}
                  >
                    <Shield className="h-4 w-4 mr-1" />
                    {permissionsState.allGranted
                      ? "View Details"
                      : "Setup Permissions"}
                  </Button>
                </div>
              </div>
            ) : (
              <div className="text-center py-4 text-muted-foreground">
                <p>Unable to check permissions status</p>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={loadPermissionsStatus}
                  className="mt-2"
                >
                  <RefreshCw className="h-4 w-4 mr-1" />
                  Try Again
                </Button>
              </div>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}