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
import { COMMANDS } from "@/lib/constants.generated";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { toast } from "sonner";
import {
  DEFAULT_PERMISSION_MODE,
  PERMISSION_FLOOR,
  PERMISSION_MODES,
  PERMISSION_NEVER_ASKS,
  parsePermissionMode,
  type PermissionMode,
} from "@/lib/permissions";
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

/**
 * How much Juno interrupts to ask.
 *
 * One row per mode, each saying what it permits. The old control was a switch
 * called "Require Tool Approval" sitting in the advanced-only Tools pane, and
 * turning it off changed nothing because a risk threshold in the agent runner
 * overruled it. Rust now has one decision function, this is the only thing that
 * sets it, and it lives in Security and Privacy where someone would look.
 */
function PermissionModeGroup() {
  const [mode, setMode] = useState<PermissionMode>(DEFAULT_PERMISSION_MODE);
  const [loading, setLoading] = useState(true);
  const [unavailable, setUnavailable] = useState(false);

  useEffect(() => {
    let active = true;
    (async () => {
      try {
        const stored = await invoke<string>(COMMANDS.TOOLS_GET_PERMISSION_MODE);
        if (active) setMode(parsePermissionMode(stored));
      } catch (error) {
        console.error("Failed to read the permission mode:", error);
        if (active) setUnavailable(true);
      } finally {
        if (active) setLoading(false);
      }
    })();
    return () => {
      active = false;
    };
  }, []);

  const handleChange = async (next: string) => {
    const chosen = parsePermissionMode(next);
    const previous = mode;
    // Optimistic: the dot moving is the feedback, so no toast on success.
    setMode(chosen);
    try {
      await invoke(COMMANDS.TOOLS_SET_PERMISSION_MODE, { mode: chosen });
    } catch (error) {
      console.error("Failed to change the permission mode:", error);
      setMode(previous);
      toast.error("Could not change that setting");
    }
  };

  return (
    <SettingsGroup
      title="Asking permission"
      footer={
        unavailable
          ? "This setting could not be loaded. Reopen Settings to try again."
          : `${PERMISSION_NEVER_ASKS} ${PERMISSION_FLOOR}`
      }
    >
      <SettingsRow
        id="permission-mode"
        label="When Juno needs permission"
        description="Pick how often Juno stops to check with you. Changing this also clears anything you told it not to ask about again."
        below={
          <RadioGroup
            value={mode}
            onValueChange={handleChange}
            disabled={loading || unavailable}
            aria-label="When Juno needs permission"
            className="gap-0 divide-y divide-border rounded-[8px] border border-border"
          >
            {PERMISSION_MODES.map((option) => (
              <Label
                key={option.value}
                htmlFor={`permission-mode-${option.value}`}
                className="flex cursor-pointer items-start gap-3 px-3 py-2.5"
              >
                <RadioGroupItem
                  id={`permission-mode-${option.value}`}
                  value={option.value}
                  className="mt-0.5 data-[state=checked]:border-[#007AFF] [&_svg]:fill-[#007AFF]"
                />
                <span className="min-w-0 space-y-0.5">
                  <span className="block text-[13px] font-medium leading-tight text-foreground">
                    {option.name}
                  </span>
                  <span className="block text-[12px] leading-snug text-muted-foreground">
                    {option.consequence}
                  </span>
                </span>
              </Label>
            ))}
          </RadioGroup>
        }
      />
    </SettingsGroup>
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

      let commandName: string;
      switch (permissionType) {
        case "accessibility":
          commandName = COMMANDS.PERMISSIONS_REQUEST_ACCESSIBILITY_PERMISSION;
          break;
        case "screen_recording":
          commandName = COMMANDS.PERMISSIONS_REQUEST_SCREEN_RECORDING_PERMISSION;
          break;
        case "microphone":
          commandName = COMMANDS.PERMISSIONS_REQUEST_MICROPHONE_PERMISSION;
          break;
        case "input_monitoring":
          commandName = COMMANDS.PERMISSIONS_REQUEST_INPUT_MONITORING_PERMISSION;
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
      <PermissionModeGroup />

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
              >
                <RefreshCw className="w-4 h-4" />
                Refresh
              </Button>
            </SettingsRow>
          )
        )}
      </SettingsGroup>
    </div>
  );
}
