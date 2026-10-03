import { Button } from "@/components/ui/button";
import { invoke } from "@tauri-apps/api/core";
import { Check, Copy } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { COMMANDS } from "@/lib/constants.generated";
import { SettingsGroup, SettingsRow } from "./ui";

interface BuildInfo {
  version: string;
  build: string;
  commit: string;
  branch: string;
  built_at: string;
  dirty: boolean;
  demo: boolean;
  cohort: string | null;
}

/**
 * Which build this is, in one copyable line.
 *
 * Every DMG used to be "Juno 0.7.0", so a bug report could only name a date
 * and two builds from the same day were indistinguishable. This says the
 * commit, and one click puts it on the clipboard for the report.
 */
export function BuildGroup() {
  const [info, setInfo] = useState<BuildInfo | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    invoke<BuildInfo>(COMMANDS.APP_GET_BUILD_INFO)
      .then(setInfo)
      .catch((error) => console.error("Failed to read build info:", error));
  }, []);

  if (!info) return null;

  const label =
    `${info.version} (${info.build}) ${info.commit}` +
    (info.dirty ? " dirty" : "") +
    (info.demo ? ` · demo${info.cohort ? ` ${info.cohort}` : ""}` : "");

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(label);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      toast.error("Could not copy the build ID");
    }
  };

  return (
    <SettingsGroup title="Build">
      <SettingsRow
        label={label}
        description={`${info.branch}, built ${new Date(info.built_at).toLocaleString()}`}
      >
        <Button variant="outline" size="sm" onClick={copy}>
          {copied ? (
            <>
              <Check className="mr-2 h-4 w-4" />
              Copied
            </>
          ) : (
            <>
              <Copy className="mr-2 h-4 w-4" />
              Copy
            </>
          )}
        </Button>
      </SettingsRow>
    </SettingsGroup>
  );
}
