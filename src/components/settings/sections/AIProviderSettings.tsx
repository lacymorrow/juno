import {
  EnvironmentVariables,
  EnvironmentVariablesHeader,
  EnvironmentVariablesTitle,
  EnvironmentVariablesToggle,
  EnvironmentVariablesContent,
  EnvironmentVariable,
} from "@/components/ai-elements/environment-variables";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Save } from "lucide-react";
import { SettingsSectionProps } from "../types";
import { SettingsGroup, SettingsRow } from "../ui";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { COMMANDS } from "@/lib/constants.generated";
import AssistantModelPicker from "./AssistantModelPicker";

/** What a demo build carries, from the backend. */
interface DemoInfo {
  is_demo: boolean;
  cohort: string | null;
}

/** What the local Claude CLI can do right now, from the backend. */
interface ClaudeCliStatus {
  available: boolean;
  authenticated: boolean;
  email: string | null;
}

export default function AIProviderSettings({ settings }: SettingsSectionProps) {
  const [demo, setDemo] = useState<DemoInfo | null>(null);
  const [cli, setCli] = useState<ClaudeCliStatus | null>(null);

  // A demo build answers with its own key, so say so rather than letting the
  // person wonder why Juno works without one.
  useEffect(() => {
    let mounted = true;
    invoke<DemoInfo>(COMMANDS.CORE_GET_DEMO_INFO)
      .then((info) => {
        if (mounted) setDemo(info);
      })
      .catch((error) => console.debug("Demo info unavailable:", error));
    return () => {
      mounted = false;
    };
  }, []);

  // Asked on open, and again whenever this window comes back. Somebody reading
  // "sign in to Claude Code", switching to a terminal to do exactly that, and
  // returning to the same sentence would have no way to know it worked.
  useEffect(() => {
    let mounted = true;
    // Focus, blur, focus fires two probes, and the slower one can land last.
    // Only the newest request is allowed to write, so a stale answer cannot
    // put "not signed in" back on screen after a fresh one cleared it.
    let latest = 0;
    const check = () => {
      const request = ++latest;
      invoke<ClaudeCliStatus>("check_claude_cli_available")
        .then((status) => {
          if (mounted && request === latest) setCli(status);
        })
        .catch((error) => console.debug("Claude CLI status unavailable:", error));
    };
    check();
    window.addEventListener("focus", check);
    return () => {
      mounted = false;
      window.removeEventListener("focus", check);
    };
  }, []);

  return (
    <div className="space-y-6">
      {demo?.is_demo && (
        <SettingsGroup
          title="Demo access"
          footer={
            demo.cohort
              ? `This is a demo copy of Juno (${demo.cohort}). Adding your own key below switches Juno to it.`
              : "This is a demo copy of Juno. Adding your own key below switches Juno to it."
          }
        >
          <SettingsRow
            label="Anthropic"
            description="Included with this build, so you can try Juno without an account"
          >
            <Badge variant="secondary">Included</Badge>
          </SettingsRow>
        </SettingsGroup>
      )}

      {/* Shared with the Models pane — one source of truth for the agent model. */}
      <AssistantModelPicker settings={settings} advanced />

      {settings.activeProvider && settings.providerSettings && (
        <SettingsGroup
          title="Provider Configuration"
          footer={`Configure settings for ${
            settings.activeProvider === "claude_cli"
              ? "Claude CLI"
              : settings.activeProvider
          }`}
        >
          <SettingsRow
            below={
              settings.activeProvider === "claude_cli" ? (
                <div className="rounded-md border border-blue-200 bg-blue-50 p-4 dark:border-blue-800 dark:bg-blue-950">
                  <p className="text-sm font-medium text-blue-900 dark:text-blue-100">
                    {cli && !cli.available
                      ? "Claude Code is not installed"
                      : cli && !cli.authenticated
                        ? "Claude Code is not signed in"
                        : "No API key needed"}
                  </p>
                  {/* Said as a fact about this machine, not as a hedge. The old
                      copy ended "if not authenticated", which left the person
                      to work out which half of the sentence applied to them. */}
                  <p className="mt-1 text-sm text-blue-700 dark:text-blue-300">
                    {cli && !cli.available ? (
                      <>
                        Juno runs on your Claude subscription through Claude Code.
                        Install it from{" "}
                        <code className="rounded bg-blue-100 px-1 py-0.5 text-xs dark:bg-blue-900">
                          claude.ai/code
                        </code>
                        , or choose another provider above.
                      </>
                    ) : cli && !cli.authenticated ? (
                      <>
                        Run{" "}
                        <code className="rounded bg-blue-100 px-1 py-0.5 text-xs dark:bg-blue-900">
                          claude login
                        </code>{" "}
                        in your terminal, then come back to this window.
                      </>
                    ) : cli?.email ? (
                      <>Juno is using your Claude subscription, signed in as {cli.email}.</>
                    ) : (
                      <>Juno is using your Claude subscription. Nothing else to set up.</>
                    )}
                  </p>
                </div>
              ) : (
                <EnvironmentVariables>
                  <EnvironmentVariablesHeader>
                    <EnvironmentVariablesTitle>API Keys</EnvironmentVariablesTitle>
                    <EnvironmentVariablesToggle />
                  </EnvironmentVariablesHeader>
                  <EnvironmentVariablesContent>
                    <EnvironmentVariable
                      name={`${(settings.activeProvider ?? "").toUpperCase()}_API_KEY`}
                      value={settings.formData.apiKey}
                      onChange={(val) =>
                        settings.setFormData((prev) => ({
                          ...prev,
                          apiKey: val,
                        }))
                      }
                      required
                    />
                  </EnvironmentVariablesContent>
                </EnvironmentVariables>
              )
            }
          />

          {/* Max tokens / temperature — not applicable to Claude CLI (managed by the CLI) */}
          {settings.activeProvider !== "claude_cli" && (
            <>
              <SettingsRow advanced htmlFor="max-tokens" label="Max Tokens">
                <Input
                  id="max-tokens"
                  type="number"
                  value={settings.formData.maxTokens}
                  onChange={(e) =>
                    settings.setFormData((prev) => ({
                      ...prev,
                      maxTokens: e.target.value,
                    }))
                  }
                  placeholder="e.g., 4000"
                  className="w-[120px]"
                />
              </SettingsRow>

              <SettingsRow advanced htmlFor="temperature" label="Temperature">
                <Input
                  id="temperature"
                  type="number"
                  step="0.1"
                  min="0"
                  max="2"
                  value={settings.formData.temperature}
                  onChange={(e) =>
                    settings.setFormData((prev) => ({
                      ...prev,
                      temperature: e.target.value,
                    }))
                  }
                  placeholder="e.g., 0.7"
                  className="w-[120px]"
                />
              </SettingsRow>
            </>
          )}

          <SettingsRow
            advanced
            htmlFor="system-prompt"
            label="System Prompt"
            below={
              <Textarea
                id="system-prompt"
                value={settings.formData.systemPrompt}
                onChange={(e) =>
                  settings.setFormData((prev) => ({
                    ...prev,
                    systemPrompt: e.target.value,
                  }))
                }
                placeholder="Enter custom system prompt (optional)"
                rows={4}
              />
            }
          />

          <SettingsRow>
            <Button onClick={settings.handleSaveProviderSettings}>
              <Save className="w-4 h-4 mr-2" />
              Save Provider Settings
            </Button>
          </SettingsRow>
        </SettingsGroup>
      )}
    </div>
  );
}
