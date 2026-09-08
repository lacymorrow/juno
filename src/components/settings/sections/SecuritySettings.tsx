import { Button } from "@/components/ui/button";
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
import { useEffect, useRef, useState } from "react";
import {
  getPermissionsStatus,
  invalidatePermissionsCache,
  type PermissionsState,
  type AppPermissionStatus,
} from "@/lib/permissions-service";
import { cn } from "@/lib/utils";
import { SettingsGroup, SettingsRow } from "../ui";

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

// Component for individual permission row (from Onboarding)
function PermissionCard({
  permission,
  permissionStatus,
  onRequest,
  isRequesting,
  isLoading,
}: {
  permission: any;
  permissionStatus: AppPermissionStatus | null;
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
        return <Eye className="w-4 h-4" />;
      case "screen-recording":
        return <Monitor className="w-4 h-4" />;
      case "microphone":
        return <Mic className="w-4 h-4" />;
      case "input-monitoring":
        return <Keyboard className="w-4 h-4" />;
      default:
        return <Shield className="w-4 h-4" />;
    }
  };

  // Show loading state
  if (isLoading) {
    return (
      <SettingsRow label={permission.title} description={permission.description}>
        <RefreshCw className="w-4 h-4 animate-spin text-muted-foreground" />
      </SettingsRow>
    );
  }

  const label = (
    <span className="flex items-center gap-2">
      <span className="text-muted-foreground">{getPermissionIcon()}</span>
      <span>{permission.title}</span>
      {isRequired && !granted && (
        <span className="rounded-full bg-muted px-2 py-0.5 text-[11px] font-medium text-muted-foreground">
          Required
        </span>
      )}
      {granted && (
        <span className="rounded-full bg-green-500/15 px-2 py-0.5 text-[11px] font-medium text-green-600 dark:text-green-400">
          Granted
        </span>
      )}
    </span>
  );

  return (
    <SettingsRow
      label={label}
      description={permission.description}
      below={
        !granted && permissionStatus ? (
          <div
            className={cn(
              "flex items-start gap-2 rounded-md border p-2 text-[12px]",
              isRequired
                ? "border-destructive/30 bg-destructive/10 text-destructive"
                : "border-yellow-500/30 bg-yellow-500/10 text-yellow-700 dark:text-yellow-400"
            )}
          >
            {isRequired ? (
              <AlertCircle className="w-3.5 h-3.5 mt-0.5 flex-shrink-0" />
            ) : (
              <Info className="w-3.5 h-3.5 mt-0.5 flex-shrink-0" />
            )}
            <span>{permissionStatus.instructions}</span>
          </div>
        ) : undefined
      }
    >
      {granted ? (
        <span className="flex items-center gap-1.5 text-[13px] font-medium text-green-600 dark:text-green-400">
          <CheckCircle className="w-4 h-4" />
          Ready to use
        </span>
      ) : (
        <Button
          onClick={onRequest}
          disabled={isRequesting}
          size="sm"
          variant={isRequired ? "default" : "outline"}
        >
          {isRequesting ? (
            <>
              <RefreshCw className="w-4 h-4 mr-2 animate-spin" />
              Opening...
            </>
          ) : (
            <>
              <Settings className="w-4 h-4 mr-2" />
              Grant Permission
            </>
          )}
        </Button>
      )}
    </SettingsRow>
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
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => { mountedRef.current = false; };
  }, []);

  // Function to check current permissions status using centralized service
  const checkPermissionsStatus = async (forceRefresh = false) => {
    try {
      setPermissionsError(null);
      setIsLoadingPermissions(true);
      // Use centralized permission service - prevents duplicate calls
      const result = await getPermissionsStatus(forceRefresh);
      setPermissionsState(result);
      // Clear any existing error when permissions check succeeds
      console.log("SecuritySettings: Updated permissions state:", result);
      return result.all_granted;
    } catch (error) {
      console.warn("Failed to check permissions status:", error);
      // Convert error to string properly
      const errorMessage = error instanceof Error ? error.message : String(error);
      setPermissionsError(errorMessage);
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

      // Invalidate cache after permission request
      invalidatePermissionsCache();

      if (granted) {
        // Permission was already granted
        await checkPermissionsStatus(true);
      } else {
        // System Settings should be open for user to grant permission
        // Wait a moment and then refresh to check if user granted it
        setTimeout(async () => {
          if (mountedRef.current) await checkPermissionsStatus(true);
        }, 2000);
      }
    } catch (error) {
      console.error(`Error requesting ${permissionType} permission:`, error);
      // Convert error to string properly
      const errorMessage = error instanceof Error ? error.message : String(error);
      setPermissionsError(errorMessage);
    } finally {
      setIsRequestingPermission(null);
    }
  };

  useEffect(() => {
    checkPermissionsStatus();
  }, []);

  return (
    <div className="space-y-6">
      <SettingsGroup
        title="macOS Permissions"
        footer="Manage system permissions required for AI computer use features"
      >
        {permissionsError && (
          <SettingsRow
            destructive
            label="Error checking permissions"
            description={permissionsError}
          />
        )}

        {/* Permission Rows */}
        {permissions.map((permission) => {
          const permissionKey = permission.id.replace(
            "-",
            "_"
          ) as keyof PermissionsState;
          const permissionStatus =
            (permissionsState?.[permissionKey] as AppPermissionStatus) || null;

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

        {/* Summary */}
        {isLoadingPermissions ? (
          <SettingsRow label="Checking permissions…">
            <RefreshCw className="w-4 h-4 animate-spin text-muted-foreground" />
          </SettingsRow>
        ) : (
          permissionsState && (
            <SettingsRow
              label={
                permissionsState.all_granted ? (
                  <span className="flex items-center gap-2 font-medium text-green-600 dark:text-green-400">
                    <CheckCircle className="w-4 h-4" />
                    All required permissions granted!
                  </span>
                ) : (
                  <span className="flex items-center gap-2 font-medium text-yellow-600 dark:text-yellow-400">
                    <AlertCircle className="w-4 h-4" />
                    Some permissions still needed
                  </span>
                )
              }
            >
              <Button
                onClick={() => {
                  checkPermissionsStatus();
                }}
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
            </SettingsRow>
          )
        )}
      </SettingsGroup>
    </div>
  );
}
