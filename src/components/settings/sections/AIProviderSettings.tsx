import {
  EnvironmentVariables,
  EnvironmentVariablesHeader,
  EnvironmentVariablesTitle,
  EnvironmentVariablesToggle,
  EnvironmentVariablesContent,
  EnvironmentVariable,
} from "@/components/ai-elements/environment-variables";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
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

  // Fields save when they lose focus, the way every other row here applies at
  // once. The group footer says "Saved" for a moment so the person knows the
  // blur did something; no button, no toast.
  const [savedAt, setSavedAt] = useState<number | null>(null);
  useEffect(() => {
    if (savedAt === null) return;
    const id = window.setTimeout(() => setSavedAt(null), 1500);
    return () => window.clearTimeout(id);
  }, [savedAt]);
  const commit = async () => {
    if (await settings.handleSaveProviderSettings()) setSavedAt(Date.now());
  };

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
      invoke<ClaudeCliStatus>(COMMANDS.ONBOARDING_CHECK_CLAUDE_CLI_AVAILABLE)
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
          footer={
            savedAt !== null
              ? "Saved."
              : settings.activeProvider === "claude_cli"
                ? "Nothing to enter. Juno talks to Claude Code on this Mac."
                : settings.activeProvider === "codex_cli"
                  ? "Nothing to enter. Juno talks to Codex on this Mac."
                  : "Changes save when you leave a field."
          }
        >
          <SettingsRow
            below={
              settings.activeProvider === "claude_cli" ? (
                <div className="rounded-md border border-border bg-muted/40 p-4">
                  <p className="text-[13px] font-medium text-foreground">
                    {cli && !cli.available
                      ? "Claude Code is not installed"
                      : cli && !cli.authenticated
                        ? "Claude Code is not signed in"
                        : "No API key needed"}
                  </p>
                  {/* Said as a fact about this machine, not as a hedge. The old
                      copy ended "if not authenticated", which left the person
                      to work out which half of the sentence applied to them. */}
                  <p className="mt-1 text-[12px] leading-snug text-muted-foreground">
                    {cli && !cli.available ? (
                      <>
                        Juno runs on your Claude subscription through Claude Code.
                        {" "}
                        {/* Not installed is a condition the person can change,
                            so the place they change it is a link they can
                            press, the way the same row in setup already does
                            it, rather than an address to retype. */}
                        <a
                          href="https://claude.ai/code"
                          target="_blank"
                          rel="noopener noreferrer"
                          className="text-[#007AFF] hover:underline dark:text-[#0A84FF]"
                        >
                          Get Claude Code
                        </a>
                        , or choose another provider above.
                      </>
                    ) : cli && !cli.authenticated ? (
                      <>
                        Run{" "}
                        <code className="rounded bg-muted px-1 py-0.5 text-xs">
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
              ) : settings.activeProvider === "codex_cli" ? (
                <div className="rounded-md border border-border bg-muted/40 p-4">
                  <p className="text-[13px] font-medium text-foreground">
                    No API key needed
                  </p>
                  <p className="mt-1 text-[12px] leading-snug text-muted-foreground">
                    Juno is using your ChatGPT plan through Codex. Nothing else
                    to set up.
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
                      onCommit={commit}
                      required
                    />
                  </EnvironmentVariablesContent>
                </EnvironmentVariables>
              )
            }
          />

          {/* Max tokens / temperature — not applicable to Claude CLI (managed by the CLI) */}
          {settings.activeProvider !== "claude_cli" &&
            settings.activeProvider !== "codex_cli" && (
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
                  onBlur={commit}
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
                  onBlur={commit}
                  placeholder="e.g., 0.7"
                  className="w-[120px]"
                />
              </SettingsRow>
            </>
          )}

          {/* Applies on the next query: one-shot spawns pick it up directly,
              and a persistent CLI session is replaced via its spawn signature. */}
          {settings.activeProvider === "claude_cli" && (
            <SettingsRow
              advanced
              htmlFor="load-account-mcp"
              label="Load account MCP connectors"
              description="Use the connectors on your Claude account, like Slack, Gmail, and Drive, in Juno chats. Off limits Juno to its own tools."
            >
              <Switch
                id="load-account-mcp"
                checked={settings.providerSettings?.load_account_mcp ?? true}
                onCheckedChange={settings.handleLoadAccountMcpChange}
              />
            </SettingsRow>
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
                onBlur={commit}
                placeholder="Optional. How Juno should behave."
                rows={4}
              />
            }
          />

        </SettingsGroup>
      )}
    </div>
  );
}
