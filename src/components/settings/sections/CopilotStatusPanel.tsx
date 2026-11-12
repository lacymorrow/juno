import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { COMMANDS } from "@/lib/constants.generated";
import { describeCopilotStatus, type CopilotStatus } from "@/lib/copilot-status";

interface CopilotStatusPanelProps {
  status: CopilotStatus | null;
  /** Called with the new status after the Copilot CLI sign-in was imported. */
  onImported: (status: CopilotStatus) => void;
}

/** The Copilot equivalent of the API key field: who Juno is signed in as. */
export default function CopilotStatusPanel({ status, onImported }: CopilotStatusPanelProps) {
  const [importing, setImporting] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);
  const text = describeCopilotStatus(status);

  const importSignIn = async () => {
    setImporting(true);
    setImportError(null);
    try {
      onImported(await invoke<CopilotStatus>(COMMANDS.PROVIDERS_IMPORT_COPILOT_CLI_SIGN_IN));
    } catch (error) {
      setImportError(String(error));
    } finally {
      setImporting(false);
    }
  };

  return (
    <div className="rounded-md border border-border bg-muted/40 p-4">
      <p className="text-[13px] font-medium text-foreground">{text.headline}</p>
      {text.detail && (
        <p className="mt-1 text-[12px] leading-snug text-muted-foreground">{text.detail}</p>
      )}
      {text.canImport && (
        <Button
          size="sm"
          variant="secondary"
          className="mt-3"
          disabled={importing}
          onClick={importSignIn}
        >
          {importing ? "Waiting for macOS…" : "Use Copilot CLI sign-in"}
        </Button>
      )}
      {importError && (
        <p className="mt-2 text-[12px] leading-snug text-destructive">{importError}</p>
      )}
    </div>
  );
}
