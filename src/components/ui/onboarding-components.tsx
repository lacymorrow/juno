/**
 * Onboarding inline JSX components rendered inside the chat window.
 *
 * These components are registered in JsxMessageRenderer so the backend can
 * emit them as JSX strings inside agent-text-stream messages. They follow the
 * display-only rule: all business logic is delegated to Rust via invoke().
 *
 * Components:
 *   OnboardingActionButton  — calls onboarding_action(next|skip|reset) + optional command
 *   PermissionStatusCard    — compact permission row with live updates via permissions-changed
 *   PermissionStatusGrid    — renders multiple PermissionStatusCard rows
 *   ProviderSelector        — inline provider picker (Claude CLI auto-detect or API key)
 *   OnboardingActions       — standard "Continue / Skip" row for each phase
 */

import { invoke } from "@tauri-apps/api/core";
import { useEventListener } from "@/hooks/useEventListener";
import { cn } from "@/lib/utils";
import { EVENTS, COMMANDS } from "@/lib/constants.generated";
import {
  Check,
  Loader2,
  X,
  Shield,
  Monitor,
  Mic,
  Keyboard,
  ChevronRight,
  Eye,
  Key,
} from "lucide-react";
import { useState, useCallback, useEffect } from "react";

// ─── Types ────────────────────────────────────────────────────────────────────

interface PermissionStatus {
  permission_type: string;
  granted: boolean;
  required: boolean;
  description: string;
}

interface PermissionsChangedPayload {
  screen_recording: PermissionStatus;
  accessibility: PermissionStatus;
  microphone: PermissionStatus;
  input_monitoring: PermissionStatus;
  all_granted: boolean;
}

// ─── OnboardingActionButton ───────────────────────────────────────────────────

interface OnboardingActionButtonProps {
  /** Onboarding state action: "next" | "skip" | "reset" */
  action?: "next" | "skip" | "reset";
  /** Optional additional Tauri command to invoke first (e.g. a permission request) */
  command?: string;
  /** Arguments for the optional command */
  args?: Record<string, unknown>;
  label?: string;
  variant?: "primary" | "secondary" | "ghost";
  className?: string;
}

export function OnboardingActionButton({
  action = "next",
  command,
  args,
  label = action === "skip" ? "Skip" : "Continue",
  variant = "primary",
  className,
}: OnboardingActionButtonProps) {
  const [status, setStatus] = useState<"idle" | "loading" | "success" | "error">("idle");

  const handleClick = useCallback(async () => {
    setStatus("loading");
    try {
      if (command) {
        await invoke(command, args ?? {});
      }
      await invoke("onboarding_action", { action });
      setStatus("success");
      setTimeout(() => setStatus("idle"), 1500);
    } catch (err) {
      console.error("[OnboardingActionButton] Failed:", err);
      setStatus("error");
      setTimeout(() => setStatus("idle"), 2500);
    }
  }, [action, command, args]);

  const variantClasses = {
    primary:
      "bg-violet-600 hover:bg-violet-700 text-white border-0 shadow-md shadow-violet-900/30",
    secondary:
      "bg-background border border-border hover:bg-accent text-foreground",
    ghost:
      "bg-transparent hover:bg-accent/50 text-muted-foreground border-0",
  };

  return (
    <button
      onClick={handleClick}
      disabled={status === "loading"}
      className={cn(
        "inline-flex items-center justify-center gap-1.5 rounded-lg px-4 h-9 text-sm font-medium transition-all",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-violet-500",
        "disabled:pointer-events-none disabled:opacity-50",
        variantClasses[variant],
        className
      )}
    >
      {status === "loading" ? (
        <Loader2 className="h-3.5 w-3.5 animate-spin" />
      ) : status === "success" ? (
        <Check className="h-3.5 w-3.5" />
      ) : null}
      {status === "success" ? "Done!" : label}
      {status === "idle" && variant === "primary" && (
        <ChevronRight className="h-3.5 w-3.5" />
      )}
    </button>
  );
}

// ─── PermissionStatusCard ─────────────────────────────────────────────────────

interface PermissionStatusCardProps {
  /** Which permission to display: "screen_recording" | "accessibility" | "microphone" | "input_monitoring" */
  permission: "screen_recording" | "accessibility" | "microphone" | "input_monitoring";
  /** Button label for the grant action */
  grantLabel?: string;
  /** Whether to show the grant button */
  showAction?: boolean;
}

const PERMISSION_META: Record<
  string,
  { label: string; icon: React.ComponentType<{ className?: string }>; command: string }
> = {
  screen_recording: {
    label: "Screen Recording",
    icon: Monitor,
    command: COMMANDS.PERMISSIONS_REQUEST_SCREEN_RECORDING_PERMISSION,
  },
  accessibility: {
    label: "Accessibility",
    icon: Shield,
    command: COMMANDS.PERMISSIONS_REQUEST_ACCESSIBILITY_PERMISSION,
  },
  microphone: {
    label: "Microphone",
    icon: Mic,
    command: COMMANDS.PERMISSIONS_REQUEST_MICROPHONE_PERMISSION,
  },
  input_monitoring: {
    label: "Input Monitoring",
    icon: Keyboard,
    command: COMMANDS.PERMISSIONS_REQUEST_INPUT_MONITORING_PERMISSION,
  },
};

export function PermissionStatusCard({
  permission,
  grantLabel = "Grant Access",
  showAction = true,
}: PermissionStatusCardProps) {
  const [granted, setGranted] = useState<boolean | null>(null);
  const [requesting, setRequesting] = useState(false);
  const meta = PERMISSION_META[permission];
  // Track whether we have already fired the live demo for this card so we never
  // play it twice on the same render (permissions-changed can fire repeatedly).
  const [demoFired, setDemoFired] = useState(false);

  // Fetch initial state
  useEffect(() => {
    invoke<{ [key: string]: PermissionStatus }>(COMMANDS.PERMISSIONS_CHECK_PERMISSIONS_STATUS)
      .then((status) => {
        const perm = status[permission];
        if (perm) setGranted(perm.granted);
      })
      .catch(() => {});
  }, [permission]);

  // Live updates from the backend poller.
  // When we see a false→true transition, run the matching live capability demo.
  useEventListener<PermissionsChangedPayload>(EVENTS.PERMISSIONS_CHANGED, (payload) => {
    const perm = payload[permission as keyof PermissionsChangedPayload];
    if (perm && typeof (perm as PermissionStatus).granted === "boolean") {
      const wasGranted = granted;
      const nowGranted = (perm as PermissionStatus).granted;
      setGranted(nowGranted);
      if (nowGranted && wasGranted === false && !demoFired) {
        setDemoFired(true);
        invoke("run_permission_demo", { permissionType: permission }).catch((err) =>
          console.warn("[PermissionStatusCard] Demo failed:", err)
        );
      }
    }
  });

  const handleGrant = useCallback(async () => {
    if (!meta) return;
    setRequesting(true);
    try {
      // Open the right System Settings pane via the existing request command.
      await invoke(meta.command);
      // Phase C: animate the onboarding cursor to the Juno toggle in System Settings.
      // Fire-and-forget — fly_and_announce internally waits ~3s for the window to appear.
      // If the user has already granted (request returned true), Settings won't open and
      // the guidance will gracefully fall back to a Tier-3 center highlight, which we
      // then dismiss almost immediately. Net cost is harmless.
      invoke("guide_to_system_settings", { permissionType: permission }).catch((err) =>
        console.warn("[PermissionStatusCard] Cursor guidance failed:", err)
      );
    } catch (err) {
      console.error("[PermissionStatusCard] Grant failed:", err);
    } finally {
      setRequesting(false);
    }
  }, [meta, permission]);

  if (!meta) return null;

  const Icon = meta.icon;

  return (
    <div
      className={cn(
        "flex items-center gap-3 rounded-lg border px-3 py-2.5 text-sm transition-colors",
        granted === true
          ? "border-emerald-500/30 bg-emerald-500/5"
          : "border-border bg-card/50"
      )}
    >
      <Icon className={cn("h-4 w-4 shrink-0", granted ? "text-emerald-500" : "text-muted-foreground")} />
      <span className="flex-1 font-medium">{meta.label}</span>

      {granted === null ? (
        <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
      ) : granted ? (
        <Check className="h-4 w-4 text-emerald-500" />
      ) : showAction ? (
        <button
          onClick={handleGrant}
          disabled={requesting}
          className={cn(
            "inline-flex items-center gap-1 rounded-md border border-violet-500/60 px-2.5 py-1 text-xs font-medium",
            "text-violet-400 hover:bg-violet-500/10 transition-colors",
            "disabled:pointer-events-none disabled:opacity-50"
          )}
        >
          {requesting ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : (
            <Eye className="h-3 w-3" />
          )}
          {grantLabel}
        </button>
      ) : (
        <X className="h-4 w-4 text-muted-foreground/50" />
      )}
    </div>
  );
}

// ─── PermissionStatusGrid ─────────────────────────────────────────────────────

interface PermissionStatusGridProps {
  /** Comma-separated list of permissions to show */
  permissions?: string;
  showActions?: boolean;
}

export function PermissionStatusGrid({
  permissions = "screen_recording,accessibility",
  showActions = true,
}: PermissionStatusGridProps) {
  const permList = permissions
    .split(",")
    .map((p) => p.trim())
    .filter(Boolean) as PermissionStatusCardProps["permission"][];

  return (
    <div className="flex flex-col gap-2">
      {permList.map((perm) => (
        <PermissionStatusCard
          key={perm}
          permission={perm}
          showAction={showActions}
        />
      ))}
    </div>
  );
}

// ─── ProviderSelector ─────────────────────────────────────────────────────────

interface ProviderSelectorProps {
  /** Which providers to show: "claude_cli,anthropic" (comma-separated) */
  providers?: string;
}

interface ProviderInfo {
  id: string;
  name: string;
  is_available: boolean;
}

export function ProviderSelector({ providers = "claude_cli,anthropic" }: ProviderSelectorProps) {
  const [providerList, setProviderList] = useState<ProviderInfo[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const wantedIds = providers.split(",").map((p) => p.trim());

  useEffect(() => {
    invoke<ProviderInfo[]>(COMMANDS.PROVIDERS_GET_PROVIDERS)
      .then((all) => {
        const filtered = all.filter((p) => wantedIds.includes(p.id));
        setProviderList(filtered);
        // Auto-select Claude CLI if available
        const cli = filtered.find((p) => p.id === "claude_cli" && p.is_available);
        if (cli) setSelected("claude_cli");
      })
      .catch(() => {});
  }, [providers]);

  const handleSelect = useCallback(async (providerId: string) => {
    setSelected(providerId);
    setError(null);
    try {
      await invoke(COMMANDS.PROVIDERS_SET_ACTIVE_PROVIDER, { providerId });
      await invoke("onboarding_action", { action: "next" });
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const handleApiKeySubmit = useCallback(async () => {
    if (!apiKey.trim() || !selected) return;
    setSaving(true);
    setError(null);
    try {
      await invoke(COMMANDS.PROVIDERS_UPDATE_PROVIDER_API_KEY, {
        providerId: selected,
        apiKey: apiKey.trim(),
      });
      await invoke(COMMANDS.PROVIDERS_SET_ACTIVE_PROVIDER, { providerId: selected });
      await invoke("onboarding_action", { action: "next" });
      setSaved(true);
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  }, [apiKey, selected]);

  return (
    <div className="flex flex-col gap-3">
      {providerList.map((provider) => {
        const isCli = provider.id === "claude_cli";
        const isSelected = selected === provider.id;

        return (
          <div
            key={provider.id}
            className={cn(
              "rounded-lg border p-3 transition-all cursor-pointer",
              isSelected ? "border-violet-500/60 bg-violet-500/5" : "border-border bg-card/50 hover:border-violet-500/30"
            )}
            onClick={() => {
              setSelected(provider.id);
              setError(null);
            }}
          >
            <div className="flex items-center gap-2.5">
              {isCli ? (
                <Monitor className="h-4 w-4 text-violet-400 shrink-0" />
              ) : (
                <Key className="h-4 w-4 text-violet-400 shrink-0" />
              )}
              <div className="flex-1 min-w-0">
                <div className="text-sm font-medium">{provider.name}</div>
                <div className="text-xs text-muted-foreground">
                  {isCli
                    ? provider.is_available
                      ? "Claude CLI detected — no API key needed"
                      : "Claude CLI not found — install claude and try again"
                    : "Enter your Anthropic API key"}
                </div>
              </div>
              {isSelected && <Check className="h-4 w-4 text-violet-500 shrink-0" />}
            </div>

            {/* API key entry for non-CLI providers */}
            {isSelected && !isCli && (
              <div className="mt-3 flex flex-col gap-2">
                <div className="relative">
                  <Key className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
                  <input
                    type="password"
                    placeholder="sk-ant-..."
                    value={apiKey}
                    onChange={(e) => setApiKey(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && handleApiKeySubmit()}
                    className={cn(
                      "w-full rounded-md border border-border bg-background pl-8 pr-3 py-2 text-sm",
                      "placeholder:text-muted-foreground/60",
                      "focus:outline-none focus:ring-2 focus:ring-violet-500/50"
                    )}
                  />
                </div>
                <button
                  onClick={handleApiKeySubmit}
                  disabled={saving || !apiKey.trim()}
                  className={cn(
                    "inline-flex items-center justify-center gap-1.5 rounded-md px-3 h-8 text-sm font-medium",
                    "bg-violet-600 hover:bg-violet-700 text-white transition-colors",
                    "disabled:pointer-events-none disabled:opacity-50"
                  )}
                >
                  {saving ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : saved ? <Check className="h-3.5 w-3.5" /> : null}
                  {saved ? "Saved!" : "Save & Continue"}
                </button>
              </div>
            )}

            {/* Use CLI immediately when available + selected */}
            {isSelected && isCli && provider.is_available && (
              <div className="mt-2 flex justify-end">
                <button
                  onClick={() => handleSelect("claude_cli")}
                  className={cn(
                    "inline-flex items-center gap-1.5 rounded-md px-3 h-8 text-sm font-medium",
                    "bg-violet-600 hover:bg-violet-700 text-white transition-colors"
                  )}
                >
                  Use Claude CLI <ChevronRight className="h-3.5 w-3.5" />
                </button>
              </div>
            )}
          </div>
        );
      })}

      {error && (
        <p className="text-xs text-destructive mt-1">{error}</p>
      )}
    </div>
  );
}

// ─── OnboardingActions ────────────────────────────────────────────────────────

interface OnboardingActionsProps {
  /** Label for the primary "Continue" button */
  continueLabel?: string;
  /** Whether to show the Skip button */
  showSkip?: boolean;
  /** Optional Tauri command to invoke before advancing */
  beforeNextCommand?: string;
  args?: string;
}

export function OnboardingActions({
  continueLabel = "Continue",
  showSkip = false,
  beforeNextCommand,
  args,
}: OnboardingActionsProps) {
  const parsedArgs = (() => {
    try {
      return args ? JSON.parse(args) : undefined;
    } catch {
      return undefined;
    }
  })();

  return (
    <div className="flex items-center gap-2 mt-2">
      <OnboardingActionButton
        action="next"
        label={continueLabel}
        variant="primary"
        command={beforeNextCommand}
        args={parsedArgs}
      />
      {showSkip && (
        <OnboardingActionButton
          action="skip"
          label="Skip"
          variant="ghost"
        />
      )}
    </div>
  );
}
