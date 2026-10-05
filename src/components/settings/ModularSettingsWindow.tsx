import { useSettingsContext } from "@/contexts/SettingsContext";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { useSystemTheme } from "@/hooks/useSystemTheme";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Bell,
  Boxes,
  Cpu,
  CalendarClock,
  Mic,
  Network,
  Search,
  Settings,
  Shield,
  Terminal,
  Wrench,
  Zap,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  GeneralSettings,
  VoiceSettings,
  AIProviderSettings,
  ModelsSettings,
  SecuritySettings,
  AdvancedSettings,
  AutomationsSettings,
  NetworkSettings,
  NotificationSettings,
  ToolsSettings,
  TriggersSettings,
} from "./index";
import {
  AdvancedSettingsProvider,
  useAdvancedSettings,
} from "./AdvancedSettingsContext";
import { SettingsCategory } from "./types";
import { SETTINGS_ROW_ID_PREFIX } from "./ui";

/**
 * Sidebar sections, styled like macOS System Settings: a coloured icon tile
 * plus a single label. `keywords` feed the sidebar search; `advanced: true`
 * hides a section until the "Advanced settings" toggle is on.
 */
interface MacCategory extends SettingsCategory {
  /** Tailwind-ready background colour for the sidebar icon tile. */
  tile: string;
  /** Extra search terms beyond the visible name. */
  keywords: string;
}

export const settingsCategories: MacCategory[] = [
  {
    id: "general",
    name: "General",
    icon: <Settings className="h-3.5 w-3.5" />,
    tile: "bg-[#8E8E93]",
    description: "Basic app settings and preferences",
    keywords: "startup launch login cursor appearance look",
  },
  {
    id: "triggers",
    name: "Triggers",
    icon: <Zap className="h-3.5 w-3.5" />,
    tile: "bg-[#00C7BE]",
    description: "How you summon Juno",
    // Carries the retired Keyboard Shortcuts pane's search terms: this is
    // where every binding lives now, so searching for one must land here.
    keywords:
      "trigger activation summon hotkey hotkeys keybindings keyboard shortcut shortcuts keys mouse button voice push to talk toggle wake word phrase agent dictation",
  },
  {
    id: "voice",
    name: "Audio",
    icon: <Mic className="h-3.5 w-3.5" />,
    tile: "bg-[#FF2D55]",
    description: "Microphone, speaker, and Juno's voice",
    keywords:
      "microphone mic speaker output input device headphones airpods voice juno's voice sound sounds cue chime dictation speech transcription say",
  },
  {
    id: "ai",
    // One pane for "which service does the work": the AI provider and the
    // voice engine. The voice itself stays under Audio.
    name: "Providers",
    icon: <Cpu className="h-3.5 w-3.5" />,
    tile: "bg-[#64D2FF]",
    description: "Which services Juno uses",
    keywords:
      "model anthropic openai gemini api key provider claude llm tts text to speech voice engine elevenlabs kokoro supertonic chatterbox",
  },
  {
    id: "models",
    name: "Models",
    icon: <Boxes className="h-3.5 w-3.5" />,
    tile: "bg-[#5856D6]",
    description: "Dictation models",
    keywords: "dictation whisper parakeet stt speech model fast balanced accurate download on device",
  },
  {
    id: "notifications",
    name: "Notifications",
    icon: <Bell className="h-3.5 w-3.5" />,
    tile: "bg-[#FF3B30]",
    description: "Alerts, sounds, and delivery",
    keywords: "alerts banners sounds badges push notify",
  },
  {
    id: "tools",
    name: "Tools",
    icon: <Wrench className="h-3.5 w-3.5" />,
    tile: "bg-[#FF9500]",
    description: "Enable and disable agent tools",
    advanced: true,
    keywords: "tools capabilities categories enable disable permissions companion observe",
  },
  {
    id: "automations",
    name: "Automations",
    icon: <CalendarClock className="h-3.5 w-3.5" />,
    tile: "bg-[#5856D6]",
    description: "Scheduled agent tasks",
    advanced: true,
    keywords: "schedule cron tasks recurring automatic jobs",
  },
  {
    id: "network",
    name: "Network",
    icon: <Network className="h-3.5 w-3.5" />,
    tile: "bg-[#007AFF]",
    description: "MCP servers and network configuration",
    advanced: true,
    keywords: "mcp servers proxy connection endpoints",
  },
  {
    id: "security",
    name: "Security & Privacy",
    icon: <Shield className="h-3.5 w-3.5" />,
    tile: "bg-[#34C759]",
    description: "Permissions and security settings",
    keywords: "permissions privacy accessibility screen recording camera microphone approvals",
  },
  {
    id: "advanced",
    name: "Advanced",
    icon: <Terminal className="h-3.5 w-3.5" />,
    tile: "bg-[#48484A]",
    description: "System settings and reset options",
    advanced: true,
    keywords:
      "reset developer logs debug data storage background dock menu bar mouse control agent mode onboarding build version commit",
  },
];

/** Categories visible for the given toggle state. Exported for tests. */
export function visibleCategories(showAdvanced: boolean): MacCategory[] {
  return settingsCategories.filter((c) => showAdvanced || !c.advanced);
}

/** Categories matching a search query (name + keywords). Exported for tests. */
export function searchCategories(
  categories: MacCategory[],
  query: string,
): MacCategory[] {
  const q = query.trim().toLowerCase();
  if (!q) return categories;
  const terms = q.split(/\s+/);
  return categories.filter((c) => {
    const haystack = `${c.name} ${c.description ?? ""} ${c.keywords}`.toLowerCase();
    return terms.every((t) => haystack.includes(t));
  });
}

/**
 * A searchable row inside a section. `rowId` matches the anchor a
 * {@link SettingsRow} renders (`id` prop, else its `htmlFor`), so a match can
 * be scrolled to and highlighted. This is a small static index maintained
 * alongside the sections — extend it when a row deserves to be findable.
 */
export interface SettingsRowEntry {
  sectionId: string;
  rowId: string;
  label: string;
  keywords: string;
  /** Only reachable while the advanced toggle is on. */
  advanced?: boolean;
}

export const settingsRowIndex: SettingsRowEntry[] = [
  // General
  { sectionId: "general", rowId: "auto-launch", label: "Open at login", keywords: "startup login boot autostart launch" },
  { sectionId: "general", rowId: "big-cursor-enabled", label: "Enable big cursor", keywords: "cursor pointer magnify enlarge big" },
  { sectionId: "general", rowId: "bar-appearance", label: "Bar appearance", keywords: "bar appearance look style pill island orb halo avatar persona floating preview" },
  { sectionId: "advanced", rowId: "restart-onboarding", label: "Restart onboarding", keywords: "onboarding welcome guide tutorial restart setup", advanced: true },
  // Triggers
  { sectionId: "triggers", rowId: "add-trigger", label: "Add trigger", keywords: "trigger activation summon hotkey shortcut mouse button voice push to talk toggle wake word phrase" },
  // Models
  { sectionId: "models", rowId: "model-row-parakeet-ctc", label: "Balanced dictation model", keywords: "parakeet balanced recommended dictation model download" },
  { sectionId: "models", rowId: "model-row-tiny-en", label: "Fast dictation model", keywords: "whisper tiny fast dictation model" },
  { sectionId: "models", rowId: "model-row-large-v3", label: "Most accurate dictation model", keywords: "whisper large accurate dictation model download" },
  // Audio
  { sectionId: "voice", rowId: "audio-input-device", label: "Listen through", keywords: "microphone mic input device listen headphones airpods usb interface" },
  { sectionId: "voice", rowId: "audio-output-device", label: "Speak through", keywords: "speaker output device sound headphones airpods play" },
  { sectionId: "voice", rowId: "juno-voice", label: "Juno's voice", keywords: "voice juno voice samantha alex daniel karen moira accent silent mute speak out loud" },
  { sectionId: "voice", rowId: "voice-engine", label: "Voice engine", keywords: "tts text to speech voice engine elevenlabs kokoro local supertonic chatterbox replicate", advanced: true },
  // Providers
  { sectionId: "ai", rowId: "ai-provider", label: "Active Provider", keywords: "provider anthropic openai gemini claude" },
  { sectionId: "ai", rowId: "max-tokens", label: "Max Tokens", keywords: "tokens length limit output", advanced: true },
  { sectionId: "ai", rowId: "temperature", label: "Temperature", keywords: "temperature randomness creativity sampling", advanced: true },
  { sectionId: "ai", rowId: "system-prompt", label: "System Prompt", keywords: "system prompt instructions persona", advanced: true },
  // Notifications
  { sectionId: "notifications", rowId: "notification-type", label: "Notification method", keywords: "banner alert method delivery" },
  { sectionId: "notifications", rowId: "position", label: "Position", keywords: "position corner placement screen" },
  // Security & Privacy. The approval row carries the retired Tools row's search
  // terms, so searching "approval" still lands on the control that works.
  { sectionId: "security", rowId: "permission-mode", label: "When Juno needs permission", keywords: "permission permissions approval approve approvals confirm ask autonomy allow always risky safe tools bash terminal don't ask" },
  { sectionId: "security", rowId: "cli-ask-before-send", label: "Ask before Juno sends", keywords: "approvals approval ask confirm send email message calendar sends", advanced: true },
  // Tools
  { sectionId: "tools", rowId: "smooth-mouse-movement", label: "Enable Smooth Mouse Movement", keywords: "smooth mouse movement animation cursor" },
  // Network
  { sectionId: "network", rowId: "mcp-json-config", label: "Server Configuration (JSON)", keywords: "mcp json server configuration endpoints" },
  // Advanced
  { sectionId: "advanced", rowId: "background-mode", label: "Work in the background", keywords: "background quiet no interruption cursor focus other apps" },
  { sectionId: "advanced", rowId: "mouse-control", label: "Mouse control", keywords: "mouse pointer cursor permission ask always takeover control" },
  { sectionId: "advanced", rowId: "show-juno-in", label: "Show Juno in", keywords: "dock icon menu bar menubar tray hide accessory app switcher missing disappeared" },
  { sectionId: "advanced", rowId: "cli-persistent-session", label: "Persistent Claude session", keywords: "claude cli persistent session process faster follow-ups" },
  { sectionId: "advanced", rowId: "debug-mode", label: "Debug Mode", keywords: "debug logs verbose developer" },
  { sectionId: "advanced", rowId: "performance-monitoring", label: "Performance Monitoring", keywords: "performance monitoring metrics profiling" },
  { sectionId: "advanced", rowId: "reset-all-settings", label: "Reset all settings", keywords: "reset factory defaults erase wipe" },
];

/**
 * Rows whose label/keywords match every term of the query. Exported for tests.
 * Empty query yields nothing — row deep-linking only kicks in on an active
 * search, section-level filtering handles the rest.
 */
export function searchRows(query: string, showAdvanced = true): SettingsRowEntry[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const terms = q.split(/\s+/);
  // A hit the person cannot reach is worse than no hit: rows behind the
  // advanced toggle, or in a section it hides, only match while it is on.
  const hiddenSections = new Set(
    settingsCategories.filter((c) => c.advanced).map((c) => c.id),
  );
  return settingsRowIndex.filter((r) => {
    if (!showAdvanced && (r.advanced || hiddenSections.has(r.sectionId))) return false;
    const haystack = `${r.label} ${r.keywords}`.toLowerCase();
    return terms.every((t) => haystack.includes(t));
  });
}

export default function ModularSettingsWindow() {
  return (
    <AdvancedSettingsProvider>
      <SettingsWindowContent />
    </AdvancedSettingsProvider>
  );
}

function SettingsWindowContent() {
  const [selectedCategory, setSelectedCategory] = useState("general");
  const [query, setQuery] = useState("");
  const settings = useSettingsContext();
  const theme = useSystemTheme();
  const contentRef = useRef<HTMLDivElement>(null);
  const { advanced, loading: advancedLoading, setAdvanced } =
    useAdvancedSettings();
  const window = getCurrentWindow();

  const categories = useMemo(() => visibleCategories(advanced), [advanced]);

  // Rows (inside sections) that match the query, restricted to visible sections.
  const rowMatches = useMemo(
    () =>
      searchRows(query, advanced).filter((r) =>
        categories.some((c) => c.id === r.sectionId),
      ),
    [categories, query],
  );

  // Sidebar list: sections matching by name/keywords, plus any section that
  // owns a matching row so a row search doesn't hide its own section.
  const filtered = useMemo(() => {
    const sectionHits = searchCategories(categories, query);
    if (rowMatches.length === 0) return sectionHits;
    const allowed = new Set([
      ...sectionHits.map((c) => c.id),
      ...rowMatches.map((r) => r.sectionId),
    ]);
    return categories.filter((c) => allowed.has(c.id));
  }, [categories, query, rowMatches]);

  // Keep the selection valid as the visible list changes (toggle off while on
  // a hidden section, or a search that hides the current one).
  useEffect(() => {
    if (!filtered.some((c) => c.id === selectedCategory)) {
      setSelectedCategory(filtered[0]?.id ?? categories[0]?.id ?? "general");
    }
  }, [filtered, categories, selectedCategory]);

  // Row-level deep-linking: when the query points at a specific row, jump to
  // its section and briefly highlight the row. The first match wins.
  useEffect(() => {
    const target = rowMatches[0];
    if (!target) return;
    setSelectedCategory(target.sectionId);

    // Wait for the target section to render, then scroll + flash the row.
    const raf = requestAnimationFrame(() => {
      const el = document.getElementById(
        `${SETTINGS_ROW_ID_PREFIX}${target.rowId}`,
      );
      if (!el) return;
      el.scrollIntoView?.({ block: "center", behavior: "smooth" });
      const ring = ["ring-2", "ring-primary/50", "rounded-md"];
      el.classList.add(...ring);
      // Plain setTimeout — the local `window` is the Tauri window, not global.
      setTimeout(() => el.classList.remove(...ring), 1600);
    });
    return () => cancelAnimationFrame(raf);
  }, [rowMatches]);

  // Scroll the content pane back to the top whenever the section changes.
  // Skip when a row is deep-linked — that effect scrolls to the row instead.
  useEffect(() => {
    if (rowMatches.length > 0) return;
    contentRef.current?.scrollTo?.({ top: 0 });
  }, [selectedCategory, rowMatches]);

  useEffect(() => {
    window.setTitle("Juno Settings").catch(() => {});
  }, [window]);

  const renderCategoryContent = () => {
    switch (selectedCategory) {
      case "general":
        return <GeneralSettings settings={settings} />;
      case "triggers":
        return <TriggersSettings settings={settings} />;
      case "voice":
        return <VoiceSettings settings={settings} />;
      case "ai":
        return <AIProviderSettings settings={settings} />;
      case "models":
        return <ModelsSettings />;
      case "notifications":
        return <NotificationSettings />;
      case "tools":
        return <ToolsSettings settings={settings} />;
      case "automations":
        return <AutomationsSettings />;
      case "network":
        return <NetworkSettings settings={settings} />;
      case "security":
        return <SecuritySettings />;
      case "advanced":
        return <AdvancedSettings settings={settings} />;
      default:
        return <GeneralSettings settings={settings} />;
    }
  };

  const current = categories.find((c) => c.id === selectedCategory);

  return (
    <div
      className={cn(theme === "dark" && "dark")}
      style={{
        fontFamily:
          '-apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", system-ui, sans-serif',
      }}
    >
      {/* Transparent body so the native window vibrancy shows through the
          translucent sidebar; the content pane stays opaque for readability. */}
      <div className="flex h-screen w-full min-w-0 bg-transparent text-foreground">
        {/* Sidebar — translucent so the macOS vibrancy blur shows through. */}
        <aside className="flex w-[220px] shrink-0 flex-col border-r border-border bg-sidebar/70">
          {/* Drag region + traffic-light clearance */}
          <div
            data-tauri-drag-region
            className="h-9 shrink-0"
          />

          {/* Search */}
          <div className="px-3 pb-2">
            <div className="relative">
              <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
              <Input
                type="text"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search"
                aria-label="Search settings"
                className="h-7 rounded-[7px] bg-black/[0.04] pl-8 text-[13px] shadow-none dark:bg-white/[0.06]"
              />
            </div>
          </div>

          {/* Category list */}
          <nav
            aria-label="Settings sections"
            className="flex-1 space-y-0.5 overflow-y-auto px-2 pb-2"
          >
            {filtered.map((category) => {
              const active = selectedCategory === category.id;
              return (
                <button
                  key={category.id}
                  onClick={() => setSelectedCategory(category.id)}
                  aria-current={active ? "page" : undefined}
                  className={cn(
                    "flex w-full items-center gap-2.5 rounded-[7px] px-2 py-1 text-left transition-colors",
                    active
                      ? "bg-accent text-accent-foreground"
                      : "text-foreground hover:bg-accent/50",
                  )}
                >
                  <span
                    className={cn(
                      "flex h-6 w-6 shrink-0 items-center justify-center rounded-[6px] text-white shadow-sm",
                      category.tile,
                    )}
                  >
                    {category.icon}
                  </span>
                  <span className="truncate text-[13px] font-medium">
                    {category.name}
                  </span>
                </button>
              );
            })}
            {filtered.length === 0 && (
              <p className="px-2 py-4 text-center text-[12px] text-muted-foreground">
                No settings found
              </p>
            )}
          </nav>

          {/* Footer: advanced toggle */}
          <div className="border-t border-border px-3 py-2.5">
            <div className="flex items-center justify-between gap-3">
              <Label
                htmlFor="advanced-settings-toggle"
                className="text-[12px] font-medium text-muted-foreground"
              >
                Show advanced settings
              </Label>
              <Switch
                id="advanced-settings-toggle"
                checked={advanced}
                onCheckedChange={(checked) => void setAdvanced(checked)}
                disabled={advancedLoading}
              />
            </div>
          </div>
        </aside>

        {/* Content */}
        <main className="flex min-w-0 flex-1 flex-col bg-background">
          <div
            data-tauri-drag-region
            className="flex h-9 shrink-0 items-center"
          />
          <div ref={contentRef} className="flex-1 overflow-y-auto">
            <div className="mx-auto max-w-[640px] px-8 pb-10">
              <h1 className="pb-4 pt-1 text-[22px] font-bold tracking-tight">
                {current?.name}
              </h1>
              {renderCategoryContent()}
            </div>
          </div>
        </main>
      </div>
    </div>
  );
}
