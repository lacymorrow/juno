import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { invoke } from "@tauri-apps/api/core";
import {
  AlertCircle,
  CheckCircle,
  RefreshCw,
  Settings,
  Shield,
  Eye,
  Monitor,
  Mic,
  Keyboard,
  Info,
} from "lucide-react";
import { useEffect, useState } from "react";

// Permission status interface matching backend (from Onboarding)
interface PermissionStatus {
  permissionType: string;
  granted: boolean;
  required: boolean;
  description: string;
  instructions: string;
}

// Complete permissions state interface (from Onboarding)
interface PermissionsState {
  accessibility: PermissionStatus;
  screenRecording: PermissionStatus;
  microphone: PermissionStatus;
  inputMonitoring: PermissionStatus;
  allGranted: boolean;
  appName: string;
}

const permissions = [
  {
    id: "accessibility",
    title: "Accessibility",
    description: "Allow Juno to control your computer",
    icon: <Eye className="w-5 h-5" />,
    required: true,
  },
  {
    id: "screen-recording",
    title: "Screen Recording",
    description: "Allow Juno to capture screen content",
    icon: <Monitor className="w-5 h-5" />,
    required: true,
  },
  {
    id: "microphone",
    title: "Microphone",
    description: "Allow Juno to use voice features",
    icon: <Mic className="w-5 h-5" />,
    required: false,
  },
  {
    id: "input-monitoring",
    title: "Input Monitoring",
    description: "Allow Juno to monitor keyboard and mouse",
    icon: <Keyboard className="w-5 h-5" />,
    required: false,
  },
];

// Component for individual permission card (from Onboarding)
function PermissionCard({
  permission,
  permissionStatus,
  onRequest,
  isRequesting,
  isLoading,
}: {
  permission: any;
  permissionStatus: PermissionStatus | null;
  onRequest: () => void;
  isRequesting: boolean;
  isLoading: boolean;
}) {
  const granted = permissionStatus?.granted ?? false;
  const isRequired = permission.required;

  // Map permission IDs to icons
  const getPermissionIcon = () => {
    switch (permission.id) {
      case "accessibility":
        return <Eye className="w-5 h-5" />;
      case "screen-recording":
        return <Monitor className="w-5 h-5" />;
      case "microphone":
        return <Mic className="w-5 h-5" />;
      case "input-monitoring":
        return <Keyboard className="w-5 h-5" />;
      default:
        return <Shield className="w-5 h-5" />;
    }
  };

  // Show loading state
  if (isLoading) {
    return (
      <div className="p-4 rounded-xl border-2 border-gray-200 bg-gray-50/30 animate-pulse">
        <div className="flex items-start gap-4">
          <div className="flex items-center gap-2">
            <div className="w-10 h-10 rounded-full bg-gray-200 flex items-center justify-center">
              <RefreshCw className="w-5 h-5 text-gray-400 animate-spin" />
            </div>
          </div>
          <div className="flex-1">
            <div className="flex items-center gap-2 mb-1">
              <div className="h-5 bg-gray-200 rounded w-24"></div>
              {isRequired && (
                <div className="h-5 bg-gray-200 rounded w-16"></div>
              )}
            </div>
            <div className="h-4 bg-gray-200 rounded w-48 mb-3"></div>
            <div className="h-8 bg-gray-200 rounded w-32"></div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div
      className={`p-4 rounded-xl border-2 transition-all duration-200 ${
        granted
          ? "border-green-200 bg-green-50/30"
          : isRequired
          ? "border-red-200 bg-red-50/30"
          : "border-yellow-200 bg-yellow-50/30"
      }`}
    >
      <div className="flex items-start gap-4">
        {/* Icon and Status */}
        <div className="flex items-center gap-2">
          <div
            className={`w-10 h-10 rounded-full flex items-center justify-center ${
              granted
                ? "bg-green-100 text-green-600"
                : isRequired
                ? "bg-red-100 text-red-600"
                : "bg-yellow-100 text-yellow-600"
            }`}
          >
            {granted ? (
              <CheckCircle className="w-5 h-5" />
            ) : (
              getPermissionIcon()
            )}
          </div>
          {granted && (
            <div className="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
          )}
        </div>

        {/* Content */}
        <div className="flex-1">
          <div className="flex items-center gap-2 mb-1">
            <h4 className="font-semibold text-gray-900">{permission.title}</h4>
            {isRequired && (
              <span className="text-xs bg-orange-100 text-orange-700 px-2 py-1 rounded-full font-medium">
                Required
              </span>
            )}
            {granted && (
              <span className="text-xs bg-green-100 text-green-700 px-2 py-1 rounded-full font-medium">
                Granted
              </span>
            )}
          </div>
          <p className="text-sm text-gray-600 mb-3">{permission.description}</p>

          {!granted && permissionStatus && (
            <div
              className={`text-xs p-2 rounded-md mb-3 ${
                isRequired
                  ? "bg-red-50 border border-red-200 text-red-700"
                  : "bg-yellow-50 border border-yellow-200 text-yellow-700"
              }`}
            >
              <div className="flex items-start gap-1">
                {isRequired ? (
                  <AlertCircle className="w-3 h-3 mt-0.5 flex-shrink-0" />
                ) : (
                  <Info className="w-3 h-3 mt-0.5 flex-shrink-0" />
                )}
                <span>{permissionStatus.instructions}</span>
              </div>
            </div>
          )}

          {/* Action Buttons */}
          {!granted && (
            <div className="flex gap-2">
              <button
                onClick={onRequest}
                disabled={isRequesting}
                className={`px-3 py-2 rounded-lg text-sm font-medium transition-all flex items-center gap-2 ${
                  isRequired
                    ? "bg-blue-600 hover:bg-blue-700 text-white"
                    : "bg-gray-600 hover:bg-gray-700 text-white"
                } disabled:opacity-50 disabled:cursor-not-allowed`}
              >
                {isRequesting ? (
                  <>
                    <RefreshCw className="w-4 h-4 animate-spin" />
                    Opening...
                  </>
                ) : (
                  <>
                    <Settings className="w-4 h-4" />
                    Grant Permission
                  </>
                )}
              </button>
            </div>
          )}

          {granted && (
            <div className="flex items-center gap-2 text-sm text-green-700">
              <CheckCircle className="w-4 h-4" />
              <span className="font-medium">Ready to use</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default function SecuritySettings() {
  // State for granular permissions (from Onboarding)
  const [permissionsState, setPermissionsState] =
    useState<PermissionsState | null>(null);
  const [isRequestingPermission, setIsRequestingPermission] = useState<
    string | null
  >(null);
  const [permissionsError, setPermissionsError] = useState<string | null>(null);
  // Add loading state for initial permission check
  const [isLoadingPermissions, setIsLoadingPermissions] =
    useState<boolean>(true);

  // Function to check current permissions status (from Onboarding)
  const checkPermissionsStatus = async () => {
    try {
      setPermissionsError(null);
      setIsLoadingPermissions(true);
      const result = await invoke<PermissionsState>(
        "check_permissions_status_native"
      );
      setPermissionsState(result);
      console.log("SecuritySettings: Updated permissions state:", result);
      return result.allGranted;
    } catch (error) {
      console.warn("Failed to check permissions status:", error);
      setPermissionsError(error as string);
      return false;
    } finally {
      setIsLoadingPermissions(false);
    }
  };

  // Individual permission request functions (from Onboarding)
  const requestPermission = async (permissionType: string) => {
    try {
      setIsRequestingPermission(permissionType);
      setPermissionsError(null);

      let commandName = "";
      switch (permissionType) {
        case "accessibility":
          commandName = "request_accessibility_permission_native";
          break;
        case "screen_recording":
          commandName = "request_screen_recording_permission_native";
          break;
        case "microphone":
          commandName = "request_microphone_permission_native";
          break;
        case "input_monitoring":
          commandName = "request_input_monitoring_permission_native";
          break;
        default:
          throw new Error(`Unknown permission type: ${permissionType}`);
      }

      const granted = await invoke<boolean>(commandName);

      if (granted) {
        // Permission was already granted
        await checkPermissionsStatus();
      } else {
        // System Settings should be open for user to grant permission
        // Wait a moment and then refresh to check if user granted it
        setTimeout(async () => {
          await checkPermissionsStatus();
        }, 2000);
      }
    } catch (error) {
      console.error(`Error requesting ${permissionType} permission:`, error);
      setPermissionsError(error as string);
    } finally {
      setIsRequestingPermission(null);
    }
  };

  useEffect(() => {
    checkPermissionsStatus();
  }, []);

  return (
    <div className="space-y-6">
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
          {permissionsError && (
            <div className="p-2 bg-red-50 border border-red-200 rounded-md">
              <p className="text-sm text-red-700">Error: {permissionsError}</p>
            </div>
          )}

          {/* Permission Cards */}
          <div className="space-y-3">
            {permissions.map((permission) => {
              const permissionKey = permission.id.replace(
                "-",
                "_"
              ) as keyof PermissionsState;
              const permissionStatus =
                (permissionsState?.[permissionKey] as PermissionStatus) || null;

              return (
                <PermissionCard
                  key={permission.id}
                  permission={permission}
                  permissionStatus={permissionStatus}
                  onRequest={() =>
                    requestPermission(permission.id.replace("-", "_"))
                  }
                  isRequesting={
                    isRequestingPermission === permission.id.replace("-", "_")
                  }
                  isLoading={isLoadingPermissions}
                />
              );
            })}
          </div>

          {/* Summary */}
          {isLoadingPermissions ? (
            <div className="mt-6 p-4 bg-gray-50 rounded-lg animate-pulse">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <div className="w-5 h-5 bg-gray-200 rounded"></div>
                  <div className="h-4 bg-gray-200 rounded w-48"></div>
                </div>
                <div className="h-8 bg-gray-200 rounded w-20"></div>
              </div>
            </div>
          ) : (
            permissionsState && (
              <div className="mt-6 p-4 bg-gray-50 rounded-lg">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    {permissionsState.allGranted ? (
                      <>
                        <CheckCircle className="w-5 h-5 text-green-600" />
                        <span className="font-medium text-green-800">
                          All required permissions granted!
                        </span>
                      </>
                    ) : (
                      <>
                        <AlertCircle className="w-5 h-5 text-orange-600" />
                        <span className="font-medium text-orange-800">
                          Some permissions still needed
                        </span>
                      </>
                    )}
                  </div>
                  <Button
                    onClick={checkPermissionsStatus}
                    variant="outline"
                    size="sm"
                    className="flex items-center gap-1"
                    disabled={isLoadingPermissions}
                  >
                    <RefreshCw
                      className={`w-4 h-4 ${
                        isLoadingPermissions ? "animate-spin" : ""
                      }`}
                    />
                    {isLoadingPermissions ? "Checking..." : "Refresh"}
                  </Button>
                </div>
              </div>
            )
          )}
        </CardContent>
      </Card>
    </div>
  );
}
