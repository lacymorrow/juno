import { useSettingsContext } from "@/contexts/SettingsContext";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { useSystemTheme } from "@/hooks/useSystemTheme";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Bell,
  Brain,
  CalendarClock,
  Keyboard,
  Mic,
  Network,
  Search,
  Settings,
  Shield,
  Terminal,
  Wrench,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  GeneralSettings,
  VoiceSettings,
  AIProviderSettings,
  SecuritySettings,
  AdvancedSettings,
  AutomationsSettings,
  NetworkSettings,
  NotificationSettings,
  ShortcutsSettings,
  ToolsSettings,
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
    keywords: "startup launch login sound onboarding companion cursor agent mode",
  },
  {
    id: "voice",
    name: "Voice & Audio",
    icon: <Mic className="h-3.5 w-3.5" />,
    tile: "bg-[#FF2D55]",
    description: "Voice transcription and audio settings",
    keywords: "microphone dictation speech transcription tts audio input output",
  },
  {
    id: "ai",
    name: "AI Provider",
    icon: <Brain className="h-3.5 w-3.5" />,
    tile: "bg-[#AF52DE]",
    description: "Configure AI models and providers",
    keywords: "model anthropic openai gemini api key provider claude llm",
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
    keywords: "tools capabilities categories enable disable permissions",
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
    id: "shortcuts",
    name: "Keyboard Shortcuts",
    icon: <Keyboard className="h-3.5 w-3.5" />,
    tile: "bg-[#64748B]",
    description: "Customize keyboard shortcuts",
    keywords: "hotkeys keybindings shortcut keys trigger",
  },
  {
    id: "advanced",
    name: "Advanced",
    icon: <Terminal className="h-3.5 w-3.5" />,
    tile: "bg-[#48484A]",
    description: "System settings and reset options",
    advanced: true,
    keywords: "reset developer logs debug data storage",
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
}

export const settingsRowIndex: SettingsRowEntry[] = [
  // General
  { sectionId: "general", rowId: "auto-launch", label: "Launch at login", keywords: "startup login boot autostart" },
  { sectionId: "general", rowId: "big-cursor-enabled", label: "Enable big cursor", keywords: "cursor pointer magnify enlarge big" },
  { sectionId: "general", rowId: "bar-appearance", label: "Bar appearance", keywords: "bar appearance style pill orb persona floating" },
  { sectionId: "general", rowId: "restart-onboarding", label: "Restart onboarding", keywords: "onboarding welcome guide tutorial restart setup" },
  // Voice & Audio
  { sectionId: "voice", rowId: "whisper-model", label: "Active Model", keywords: "whisper model transcription download" },
  { sectionId: "voice", rowId: "always-listening", label: "Enable Always Listening", keywords: "wake word always listening hands-free" },
  { sectionId: "voice", rowId: "dictation-trigger-mode", label: "Trigger Mode", keywords: "dictation trigger tap hold" },
  // AI Provider
  { sectionId: "ai", rowId: "ai-provider", label: "Active Provider", keywords: "provider anthropic openai gemini claude" },
  { sectionId: "ai", rowId: "max-tokens", label: "Max Tokens", keywords: "tokens length limit output" },
  { sectionId: "ai", rowId: "temperature", label: "Temperature", keywords: "temperature randomness creativity sampling" },
  { sectionId: "ai", rowId: "system-prompt", label: "System Prompt", keywords: "system prompt instructions persona" },
  // Notifications
  { sectionId: "notifications", rowId: "notification-type", label: "Notification method", keywords: "banner alert method delivery" },
  { sectionId: "notifications", rowId: "position", label: "Position", keywords: "position corner placement screen" },
  // Tools
  { sectionId: "tools", rowId: "tool-approval-required", label: "Require Tool Approval", keywords: "approval confirm permission tools" },
  { sectionId: "tools", rowId: "smooth-mouse-movement", label: "Enable Smooth Mouse Movement", keywords: "smooth mouse movement animation cursor" },
  // Network
  { sectionId: "network", rowId: "mcp-json-config", label: "Server Configuration (JSON)", keywords: "mcp json server configuration endpoints" },
  // Advanced
  { sectionId: "advanced", rowId: "debug-mode", label: "Debug Mode", keywords: "debug logs verbose developer" },
  { sectionId: "advanced", rowId: "performance-monitoring", label: "Performance Monitoring", keywords: "performance monitoring metrics profiling" },
  { sectionId: "advanced", rowId: "reset-all-settings", label: "Reset all settings", keywords: "reset factory defaults erase wipe" },
];

/**
 * Rows whose label/keywords match every term of the query. Exported for tests.
 * Empty query yields nothing — row deep-linking only kicks in on an active
 * search, section-level filtering handles the rest.
 */
export function searchRows(query: string): SettingsRowEntry[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const terms = q.split(/\s+/);
  return settingsRowIndex.filter((r) => {
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
      searchRows(query).filter((r) =>
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
      case "voice":
        return <VoiceSettings settings={settings} />;
      case "ai":
        return <AIProviderSettings settings={settings} />;
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
      case "shortcuts":
        return <ShortcutsSettings settings={settings} />;
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
