/**
 * Permission diagnostics.
 *
 * One job: say what macOS answers about this process right now, per
 * permission. It cannot read the checkbox in System Settings, so it never
 * claims to. It reports the live answer, names the API that gave it, and
 * links to the pane so the two can be compared by the person.
 *
 * Every decision below is the backend's. This file renders the report and
 * sends back one of two intents: recheck, or reset this one permission.
 */

import { Button } from "@/components/ui/button";
import { invokeCommand } from "@/lib/utils";
import { RefreshCw } from "lucide-react";
import React, { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";

type LiveAnswer =
  | "granted"
  | "denied"
  | "not_determined"
  | "unreadable"
  | "not_applicable";

interface PermissionDiagnostic {
  permission_type: string;
  label: string;
  required: boolean;
  answer: LiveAnswer;
  answer_detail: string;
  api: string;
  tcc_service: string;
  settings_url: string;
  settings_label: string;
  gates: string;
  reset_this_launch: boolean;
}

interface PermissionDiagnosticsReport {
  bundle_id: string;
  app_name: string;
  app_version: string;
  debug_build: boolean;
  platform_supported: boolean;
  reset_available: boolean;
  reset_unavailable_reason: string | null;
  relaunch_available: boolean;
  permissions: PermissionDiagnostic[];
}

interface PermissionResetOutcome {
  permission_type: string;
  tcc_service: string;
  bundle_id: string;
  succeeded: boolean;
  message: string;
  relaunch_required: boolean;
}

const ANSWER_LABEL: Record<LiveAnswer, string> = {
  granted: "Granted",
  denied: "Not granted",
  not_determined: "Never asked",
  unreadable: "Could not read",
  not_applicable: "Not applicable",
};

/**
 * Only a grant gets the accent. Everything else is neutral on purpose: a
 * permission that is not granted is a normal state, not an error, and colouring
 * it like one would make the panel shout at a machine that is working fine.
 */
const dotClass = (answer: LiveAnswer): string =>
  answer === "granted" ? "bg-primary" : "bg-muted-foreground/40";

const PermissionDiagnostics: React.FC = () => {
  const [report, setReport] = useState<PermissionDiagnosticsReport | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [readAt, setReadAt] = useState<string>("");
  const [confirming, setConfirming] = useState<string | null>(null);
  const [resetting, setResetting] = useState<string | null>(null);
  const [outcomes, setOutcomes] = useState<Record<string, PermissionResetOutcome>>({});

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const next = await invokeCommand<PermissionDiagnosticsReport>(
        "get_permission_diagnostics",
        {}
      );
      setReport(next);
      setLoadError(null);
      setReadAt(new Date().toLocaleTimeString());
    } catch (error) {
      setLoadError(String(error));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const handleReset = async (permission: PermissionDiagnostic) => {
    setResetting(permission.permission_type);
    try {
      const outcome = await invokeCommand<PermissionResetOutcome>(
        "reset_permission_grant",
        { permissionType: permission.permission_type }
      );
      setOutcomes((prev) => ({ ...prev, [permission.permission_type]: outcome }));
      setConfirming(null);
      if (outcome.succeeded) {
        toast.success(`${permission.label} reset. Relaunch Juno to read the new state.`);
      } else {
        toast.error(outcome.message);
      }
      await load();
    } catch (error) {
      toast.error(`Could not reset ${permission.label}: ${String(error)}`);
    } finally {
      setResetting(null);
    }
  };

  const handleOpenSettings = async (permission: PermissionDiagnostic) => {
    try {
      await invokeCommand("open_system_settings_enhanced", {
        permissionType: permission.permission_type,
      });
    } catch (error) {
      toast.error(`Could not open ${permission.settings_label}: ${String(error)}`);
    }
  };

  const handleRelaunch = async () => {
    try {
      await invokeCommand("restart_app_after_permissions", {});
    } catch (error) {
      toast.error(`Could not relaunch Juno: ${String(error)}`);
    }
  };

  // Loading, first read.
  if (loading && !report && !loadError) {
    return (
      <p className="text-sm text-muted-foreground">
        Reading what macOS says about this process.
      </p>
    );
  }

  // Error state. The panel failed, the permissions did not.
  if (loadError) {
    return (
      <div className="space-y-3">
        <p className="text-sm">Could not read the permission state.</p>
        <p className="text-xs text-muted-foreground">{loadError}</p>
        <Button size="sm" onClick={() => void load()}>
          Try again
        </Button>
      </div>
    );
  }

  if (!report) {
    return null;
  }

  // Empty state. There is no TCC to ask outside macOS.
  if (!report.platform_supported) {
    return (
      <p className="text-sm text-muted-foreground">
        Permission diagnostics are macOS only. This build has no TCC to ask.
      </p>
    );
  }

  return (
    <div className="space-y-4">
      {/* Who is asking. Both lines matter to a stale grant: TCC keys a grant to
          the bundle id and the code signature, and a debug build is re-signed
          on every rebuild. */}
      <div className="flex items-start justify-between gap-4">
        <div className="space-y-1">
          <p className="text-sm">
            {report.app_name} {report.app_version}
          </p>
          <p className="font-mono text-xs text-muted-foreground">{report.bundle_id}</p>
          <p className="text-xs text-muted-foreground">
            {report.debug_build
              ? "Debug build. Re-signed on every rebuild, so an old grant can stop matching."
              : "Release build."}
            {readAt ? ` Read at ${readAt}.` : ""}
          </p>
        </div>
        <Button size="sm" onClick={() => void load()} disabled={loading}>
          <RefreshCw className="mr-2 h-3.5 w-3.5" />
          {loading ? "Reading" : "Recheck"}
        </Button>
      </div>

      <div className="divide-y rounded-md border">
        {report.permissions.map((permission) => {
          const outcome = outcomes[permission.permission_type];
          const isConfirming = confirming === permission.permission_type;
          const isResetting = resetting === permission.permission_type;
          const stale = permission.reset_this_launch;

          return (
            <div key={permission.permission_type} className="space-y-2 p-3">
              <div className="flex items-baseline justify-between gap-4">
                <div className="space-y-1">
                  <p className="text-sm">
                    {permission.label}
                    {permission.required ? (
                      <span className="ml-2 text-xs text-muted-foreground">Required</span>
                    ) : null}
                  </p>
                  <p className="text-xs text-muted-foreground">{permission.gates}</p>
                </div>
                <div className="flex shrink-0 items-center gap-2">
                  <span
                    aria-hidden="true"
                    className={`h-2 w-2 rounded-full ${dotClass(permission.answer)}`}
                  />
                  <span className="text-sm">{ANSWER_LABEL[permission.answer]}</span>
                </div>
              </div>

              <p className="text-xs text-muted-foreground">{permission.answer_detail}</p>

              <p className="font-mono text-[11px] text-muted-foreground">
                {permission.api} &middot; TCC service {permission.tcc_service}
              </p>

              {/* The honest version of "the checkbox is on but it does not
                  work". We cannot read the checkbox, so we say what we can
                  check and leave the comparison to the person. */}
              {permission.answer === "denied" && report.debug_build ? (
                <p className="text-xs text-muted-foreground">
                  If {permission.settings_label} already shows Juno switched on, that row
                  belongs to an older signature of this build and macOS is not counting it.
                </p>
              ) : null}

              {stale ? (
                <p className="text-xs text-muted-foreground">
                  Reset during this launch. The answer above is the one this process was
                  given before the reset and cannot change until Juno relaunches.
                </p>
              ) : null}

              {outcome ? (
                <p className="text-xs text-muted-foreground">{outcome.message}</p>
              ) : null}

              {isConfirming ? (
                <div className="space-y-2 rounded-md border p-3">
                  <p className="text-sm">
                    Revoke {permission.label} for {report.bundle_id}?
                  </p>
                  <p className="text-xs text-muted-foreground">
                    This clears the TCC record for this bundle id only. You approve{" "}
                    {permission.label} again in {permission.settings_label}, and Juno has to
                    relaunch before it can use it.
                  </p>
                  <div className="flex gap-2">
                    <Button
                      size="sm"
                      variant="destructive"
                      disabled={isResetting}
                      onClick={() => void handleReset(permission)}
                    >
                      {isResetting ? "Resetting" : `Reset ${permission.label}`}
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={isResetting}
                      onClick={() => setConfirming(null)}
                    >
                      Cancel
                    </Button>
                  </div>
                </div>
              ) : (
                <div className="flex flex-wrap items-center gap-3">
                  <button
                    type="button"
                    className="text-xs text-primary hover:underline"
                    onClick={() => void handleOpenSettings(permission)}
                  >
                    Open {permission.settings_label}
                  </button>
                  {report.reset_available ? (
                    <button
                      type="button"
                      className="text-xs text-muted-foreground hover:underline"
                      onClick={() => setConfirming(permission.permission_type)}
                    >
                      Reset this grant
                    </button>
                  ) : null}
                  {stale && report.relaunch_available ? (
                    <button
                      type="button"
                      className="text-xs text-primary hover:underline"
                      onClick={() => void handleRelaunch()}
                    >
                      Relaunch Juno
                    </button>
                  ) : null}
                </div>
              )}
            </div>
          );
        })}
      </div>

      {/* The one paragraph that explains the gap this panel exists for. */}
      <p className="text-xs text-muted-foreground">
        macOS keys a grant to this bundle id and this build's code signature. System
        Settings shows a checkbox for the app; the answers above are what macOS tells this
        running process. The two can disagree, and a development build is where that
        happens, because every rebuild re-signs it and leaves the old switched-on row
        behind. This panel reports only the live answer. It cannot read the checkbox.
      </p>

      {!report.reset_available && report.reset_unavailable_reason ? (
        <p className="text-xs text-muted-foreground">{report.reset_unavailable_reason}</p>
      ) : null}
    </div>
  );
};

export default PermissionDiagnostics;
