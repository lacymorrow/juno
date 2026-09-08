import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Command,
  Keyboard,
  Mic,
  MousePointer2,
  Plus,
  RefreshCw,
  Trash2,
  Zap,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";

import { SettingsSectionProps } from "../types";
import { SettingsGroup } from "../ui";
import ShortcutInput from "../ShortcutInput";

/* -------------------------------------------------------------------------- */
/* Backend contract (frozen — mirror the serde shape exactly)                 */
/* -------------------------------------------------------------------------- */

type TriggerMethod = "push_to_talk" | "toggle" | "voice";
type TriggerTarget = "agent" | "dictation";

type Binding =
  | { kind: "keyboard"; shortcut: string }
  | { kind: "mouse"; button: number };

interface Trigger {
  method: TriggerMethod;
  target: TriggerTarget;
  binding: Binding | null;
  phrase: string | null;
  require_hey_prefix: boolean;
  enabled: boolean;
}

/* -------------------------------------------------------------------------- */
/* Labels + helpers                                                           */
/* -------------------------------------------------------------------------- */

const METHOD_LABEL: Record<TriggerMethod, string> = {
  push_to_talk: "Push-to-talk",
  toggle: "Toggle",
  voice: "Voice",
};

const TARGET_LABEL: Record<TriggerTarget, string> = {
  agent: "Agent",
  dictation: "Dictation",
};

const METHOD_HINT: Record<TriggerMethod, string> = {
  push_to_talk: "Hold the binding while you speak or type.",
  toggle: "Press once to start, again to stop.",
  voice: "Say the phrase out loud to summon Juno.",
};

const ALL_METHODS: TriggerMethod[] = ["push_to_talk", "toggle", "voice"];
const ALL_TARGETS: TriggerTarget[] = ["agent", "dictation"];

const triggerKey = (t: Pick<Trigger, "method" | "target">) =>
  `${t.method}:${t.target}`;

const comboLabel = (method: TriggerMethod, target: TriggerTarget) =>
  `${METHOD_LABEL[method]} → ${TARGET_LABEL[target]}`;

const errStr = (e: unknown) =>
  typeof e === "string" ? e : e instanceof Error ? e.message : String(e);

/**
 * AppKit mouse-button numbering → a human label.
 * (0 left, 1 right, 2 middle; 3+ are physical side buttons, shown 1-indexed.)
 */
function mouseLabel(button: number): string {
  switch (button) {
    case 0:
      return "Left Click";
    case 1:
      return "Right Click";
    case 2:
      return "Middle Click";
    default:
      return `Mouse Button ${button + 1}`;
  }
}

function bindingLabel(binding: Binding | null): string {
  if (!binding) return "Set binding";
  if (binding.kind === "keyboard") return binding.shortcut || "Set binding";
  return mouseLabel(binding.button);
}

/**
 * Browser MouseEvent.button → AppKit buttonNumber.
 * Browser: 0 left, 1 middle, 2 right, 3 back, 4 forward.
 * AppKit:  0 left, 1 right,  2 middle, 3+ side.
 */
function browserButtonToAppKit(button: number): number {
  const map: Record<number, number> = { 0: 0, 1: 2, 2: 1 };
  return map[button] ?? button;
}

function defaultsFor(method: TriggerMethod, target: TriggerTarget): Trigger {
  if (method === "voice") {
    return {
      method,
      target,
      binding: null,
      phrase: target === "agent" ? "juno" : "transcribe",
      require_hey_prefix: false,
      enabled: true,
    };
  }
  return {
    method,
    target,
    binding: null,
    phrase: null,
    require_hey_prefix: false,
    enabled: true,
  };
}

/* -------------------------------------------------------------------------- */
/* Screen                                                                     */
/* -------------------------------------------------------------------------- */

export default function TriggersSettings({ settings }: SettingsSectionProps) {
  const [triggers, setTriggers] = useState<Trigger[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);

  // Inline conflict/error, pinned to the row that caused it.
  const [rowError, setRowError] = useState<{ key: string; message: string } | null>(
    null,
  );

  // Which key row has its binding editor open, and which capture tab it shows.
  const [editingKey, setEditingKey] = useState<string | null>(null);
  const [bindingTab, setBindingTab] = useState<Record<string, "keyboard" | "mouse">>(
    {},
  );

  // Last list the backend accepted — the safe target to revert to on conflict.
  const persistedRef = useRef<Trigger[]>([]);
  // Latest rendered list, so debounced/immediate saves never read stale state.
  const triggersRef = useRef<Trigger[]>([]);
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    triggersRef.current = triggers;
  }, [triggers]);

  const load = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const list = await invoke<Trigger[]>("get_triggers");
      persistedRef.current = list;
      setTriggers(list);
    } catch (e) {
      setLoadError(errStr(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
    return () => {
      if (saveTimer.current) clearTimeout(saveTimer.current);
    };
  }, [load]);

  /**
   * Persist the whole list. Optimistic: `triggers` already reflects `next`.
   * On success we adopt the backend's normalized list; on a conflict we revert
   * to the last accepted list and surface the error on `editedKey`, then
   * rethrow so callers (e.g. ShortcutInput) can react.
   */
  const applyTriggers = useCallback(
    async (next: Trigger[], editedKey: string) => {
      try {
        const normalized = await invoke<Trigger[]>("set_triggers", {
          triggers: next,
        });
        persistedRef.current = normalized;
        setTriggers(normalized);
        setRowError((prev) => (prev?.key === editedKey ? null : prev));
      } catch (e) {
        setTriggers(persistedRef.current);
        setRowError({ key: editedKey, message: errStr(e) });
        throw e;
      }
    },
    [],
  );

  const scheduleSave = useCallback(
    (next: Trigger[], editedKey: string) => {
      if (saveTimer.current) clearTimeout(saveTimer.current);
      saveTimer.current = setTimeout(() => {
        void applyTriggers(next, editedKey);
      }, 300);
    },
    [applyTriggers],
  );

  const patchTrigger = useCallback(
    (
      key: string,
      patch: Partial<Trigger>,
      opts: { immediate?: boolean } = {},
    ) => {
      const next = triggersRef.current.map((t) =>
        triggerKey(t) === key ? { ...t, ...patch } : t,
      );
      setTriggers(next);
      if (opts.immediate) void applyTriggers(next, key);
      else scheduleSave(next, key);
    },
    [applyTriggers, scheduleSave],
  );

  const addTrigger = useCallback(
    (method: TriggerMethod, target: TriggerTarget) => {
      const created = defaultsFor(method, target);
      const key = triggerKey(created);
      const next = [...triggers, created];
      setTriggers(next);
      void applyTriggers(next, key);
      // Enter the record flow for key methods; voice edits inline in place.
      if (method !== "voice") {
        setBindingTab((prev) => ({ ...prev, [key]: "keyboard" }));
        setEditingKey(key);
      }
    },
    [triggers, applyTriggers],
  );

  const removeTrigger = useCallback(
    (key: string) => {
      const next = triggers.filter((t) => triggerKey(t) !== key);
      setTriggers(next);
      if (editingKey === key) setEditingKey(null);
      setRowError((prev) => (prev?.key === key ? null : prev));
      void applyTriggers(next, key);
    },
    [triggers, editingKey, applyTriggers],
  );

  const present = new Set(triggers.map(triggerKey));
  const remaining: Array<{ method: TriggerMethod; target: TriggerTarget }> = [];
  for (const method of ALL_METHODS)
    for (const target of ALL_TARGETS)
      if (!present.has(`${method}:${target}`)) remaining.push({ method, target });

  /* --------------------------- loading / error --------------------------- */

  if (loading) {
    return (
      <div className="space-y-6">
        <SettingsGroup title="Triggers">
          <div className="flex items-center justify-center py-10 text-muted-foreground">
            <RefreshCw className="h-5 w-5 animate-spin" />
            <span className="ml-2 text-[13px]">Loading triggers…</span>
          </div>
        </SettingsGroup>
      </div>
    );
  }

  if (loadError) {
    return (
      <div className="space-y-6">
        <SettingsGroup title="Triggers">
          <div className="flex flex-col items-center gap-3 px-4 py-10 text-center">
            <p className="text-[13px] text-muted-foreground">
              Couldn’t load your triggers. {loadError}
            </p>
            <Button variant="outline" size="sm" onClick={() => void load()}>
              <RefreshCw className="size-3.5" />
              Try again
            </Button>
          </div>
        </SettingsGroup>
      </div>
    );
  }

  const addMenu = (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button size="sm" disabled={remaining.length === 0}>
          <Plus className="size-3.5" />
          Add trigger
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-56">
        {remaining.map(({ method, target }) => (
          <DropdownMenuItem
            key={`${method}:${target}`}
            onSelect={() => addTrigger(method, target)}
          >
            {method === "voice" ? (
              <Mic className="size-3.5 text-muted-foreground" />
            ) : (
              <Keyboard className="size-3.5 text-muted-foreground" />
            )}
            {comboLabel(method, target)}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );

  /* ------------------------------ empty state ---------------------------- */

  if (triggers.length === 0) {
    return (
      <div className="space-y-6">
        <SettingsGroup title="Triggers">
          <div className="flex flex-col items-center gap-4 px-6 py-12 text-center">
            <span className="flex h-12 w-12 items-center justify-center rounded-2xl bg-[#00C7BE]/15 text-[#00C7BE]">
              <Zap className="h-6 w-6" />
            </span>
            <div className="space-y-1">
              <p className="text-[14px] font-semibold">No triggers yet</p>
              <p className="text-[12px] leading-snug text-muted-foreground">
                A trigger is how you summon Juno — a hotkey, a mouse button, or
                your voice.
              </p>
            </div>
            {addMenu}
          </div>
        </SettingsGroup>
      </div>
    );
  }

  /* ------------------------------- list ---------------------------------- */

  return (
    <div className="space-y-6">
      <SettingsGroup
        title="Triggers"
        footer="Each way to summon Juno is one trigger. Add one of each method for the agent and for dictation."
      >
        {triggers.map((trigger) => (
          <TriggerRow
            key={triggerKey(trigger)}
            trigger={trigger}
            editing={editingKey === triggerKey(trigger)}
            bindingTab={bindingTab[triggerKey(trigger)] ?? "keyboard"}
            rowError={
              rowError?.key === triggerKey(trigger) ? rowError.message : null
            }
            alwaysListening={Boolean(settings.alwaysListeningActive)}
            onOpenEditor={() => {
              const key = triggerKey(trigger);
              setBindingTab((prev) => ({
                ...prev,
                [key]: trigger.binding?.kind === "mouse" ? "mouse" : "keyboard",
              }));
              setEditingKey(key);
            }}
            onCloseEditor={() => setEditingKey(null)}
            onTabChange={(tab) =>
              setBindingTab((prev) => ({
                ...prev,
                [triggerKey(trigger)]: tab,
              }))
            }
            onPatch={patchTrigger}
            onApplyBinding={applyTriggers}
            currentList={triggers}
            onRemove={() => removeTrigger(triggerKey(trigger))}
          />
        ))}
      </SettingsGroup>

      <div className="flex items-center justify-between px-1">
        <p className="text-[12px] text-muted-foreground">
          {remaining.length === 0
            ? "Every trigger type is in use."
            : `${remaining.length} more ${
                remaining.length === 1 ? "combination" : "combinations"
              } available.`}
        </p>
        {addMenu}
      </div>
    </div>
  );
}

/* -------------------------------------------------------------------------- */
/* Row                                                                        */
/* -------------------------------------------------------------------------- */

interface TriggerRowProps {
  trigger: Trigger;
  editing: boolean;
  bindingTab: "keyboard" | "mouse";
  rowError: string | null;
  alwaysListening: boolean;
  onOpenEditor: () => void;
  onCloseEditor: () => void;
  onTabChange: (tab: "keyboard" | "mouse") => void;
  onPatch: (
    key: string,
    patch: Partial<Trigger>,
    opts?: { immediate?: boolean },
  ) => void;
  onApplyBinding: (next: Trigger[], editedKey: string) => Promise<void>;
  currentList: Trigger[];
  onRemove: () => void;
}

function TriggerRow({
  trigger,
  editing,
  bindingTab,
  rowError,
  alwaysListening,
  onOpenEditor,
  onCloseEditor,
  onTabChange,
  onPatch,
  onApplyBinding,
  currentList,
  onRemove,
}: TriggerRowProps) {
  const key = triggerKey(trigger);
  const isVoice = trigger.method === "voice";

  const MethodIcon = isVoice
    ? Mic
    : trigger.binding?.kind === "mouse"
      ? MousePointer2
      : trigger.method === "toggle"
        ? Command
        : Keyboard;

  const setBindingNow = async (binding: Binding | null) => {
    const next = currentList.map((t) =>
      triggerKey(t) === key ? { ...t, binding } : t,
    );
    await onApplyBinding(next, key);
  };

  return (
    <div className="px-4 py-3">
      <div className="flex items-center gap-3">
        <span
          className={cn(
            "flex h-7 w-7 shrink-0 items-center justify-center rounded-[7px]",
            trigger.enabled
              ? "bg-[#00C7BE]/15 text-[#00C7BE]"
              : "bg-muted text-muted-foreground",
          )}
        >
          <MethodIcon className="h-3.5 w-3.5" />
        </span>

        <div className={cn("min-w-0 flex-1", !trigger.enabled && "opacity-55")}>
          <div className="flex items-baseline gap-1.5">
            <span className="truncate text-[13px] font-medium">
              {METHOD_LABEL[trigger.method]}
            </span>
            <span className="shrink-0 text-[13px] text-muted-foreground">
              → {TARGET_LABEL[trigger.target]}
            </span>
          </div>
          <p className="truncate text-[12px] leading-snug text-muted-foreground">
            {METHOD_HINT[trigger.method]}
          </p>
        </div>

        {/* Binding chip (key methods only) */}
        {!isVoice && (
          <Button
            variant="outline"
            size="xs"
            onClick={() => (editing ? onCloseEditor() : onOpenEditor())}
            aria-label={`Edit binding for ${comboLabel(
              trigger.method,
              trigger.target,
            )}`}
            className={cn(
              "font-mono",
              !trigger.binding && "text-muted-foreground",
            )}
          >
            {trigger.binding?.kind === "mouse" ? (
              <MousePointer2 className="size-3" />
            ) : (
              <Keyboard className="size-3" />
            )}
            {bindingLabel(trigger.binding)}
          </Button>
        )}

        <Switch
          checked={trigger.enabled}
          onCheckedChange={(checked) =>
            onPatch(key, { enabled: checked }, { immediate: true })
          }
          aria-label={`Enable ${comboLabel(trigger.method, trigger.target)}`}
        />

        <Button
          variant="ghost"
          size="icon-xs"
          onClick={onRemove}
          aria-label={`Delete ${comboLabel(trigger.method, trigger.target)}`}
          className="text-muted-foreground hover:text-destructive"
        >
          <Trash2 className="size-3.5" />
        </Button>
      </div>

      {/* Voice controls (always shown for voice rows) */}
      {isVoice && (
        <div className={cn("mt-3 space-y-2", !trigger.enabled && "opacity-55")}>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() =>
                onPatch(
                  key,
                  { require_hey_prefix: !trigger.require_hey_prefix },
                  { immediate: true },
                )
              }
              aria-pressed={trigger.require_hey_prefix}
              aria-label="Require the “hey” prefix"
              className={cn(
                "shrink-0 rounded-md border px-2 py-1.5 font-mono text-[13px] transition-colors",
                trigger.require_hey_prefix
                  ? "border-[#00C7BE]/50 bg-[#00C7BE]/10 text-foreground"
                  : "border-transparent text-muted-foreground/50 hover:text-muted-foreground",
              )}
            >
              hey
            </button>
            <Input
              value={trigger.phrase ?? ""}
              onChange={(e) => onPatch(key, { phrase: e.target.value })}
              placeholder={trigger.target === "agent" ? "juno" : "transcribe"}
              aria-label="Wake phrase"
              className="h-8 flex-1 font-mono text-[13px]"
            />
            <Switch
              checked={trigger.require_hey_prefix}
              onCheckedChange={(checked) =>
                onPatch(key, { require_hey_prefix: checked }, { immediate: true })
              }
              aria-label="Require the hey prefix"
            />
          </div>

          <div className="flex items-center justify-between gap-2">
            <p className="text-[12px] text-muted-foreground">
              Say{" "}
              <span className="font-medium text-foreground">
                {trigger.require_hey_prefix
                  ? `“hey ${trigger.phrase || "…"}”`
                  : `“${trigger.phrase || "…"}” or “hey ${trigger.phrase || "…"}”`}
              </span>
            </p>
            <span
              className={cn(
                "flex shrink-0 items-center gap-1.5 text-[11px]",
                alwaysListening ? "text-[#34C759]" : "text-muted-foreground",
              )}
            >
              <span
                className={cn(
                  "h-1.5 w-1.5 rounded-full",
                  alwaysListening ? "bg-[#34C759]" : "bg-muted-foreground/50",
                )}
              />
              {alwaysListening ? "Listening" : "Needs mic + always-listening"}
            </span>
          </div>
        </div>
      )}

      {/* Binding editor (key methods, when open) */}
      {!isVoice && editing && (
        <div className="mt-3 space-y-3 rounded-md border bg-muted/30 p-2.5">
          <div className="inline-flex rounded-md border p-0.5 text-[12px]">
            {(["keyboard", "mouse"] as const).map((tab) => (
              <button
                key={tab}
                type="button"
                onClick={() => onTabChange(tab)}
                className={cn(
                  "rounded px-2.5 py-1 font-medium transition-colors",
                  bindingTab === tab
                    ? "bg-background shadow-sm"
                    : "text-muted-foreground hover:text-foreground",
                )}
              >
                {tab === "keyboard" ? "Keyboard" : "Mouse"}
              </button>
            ))}
          </div>

          {bindingTab === "keyboard" ? (
            <ShortcutInput
              label="Keyboard shortcut"
              description="Press the key combination that summons this trigger."
              value={
                trigger.binding?.kind === "keyboard"
                  ? trigger.binding.shortcut
                  : ""
              }
              shortcutName={`trigger_${key}`}
              isSystemManaged={false}
              isLoading={false}
              onSave={async (_name, value) => {
                await setBindingNow(
                  value ? { kind: "keyboard", shortcut: value } : null,
                );
                onCloseEditor();
              }}
            />
          ) : (
            <MouseCapture
              current={trigger.binding?.kind === "mouse" ? trigger.binding : null}
              onCapture={async (button) => {
                await setBindingNow({ kind: "mouse", button });
                onCloseEditor();
              }}
              onCancel={onCloseEditor}
            />
          )}
        </div>
      )}

      {rowError && (
        <p className="mt-2 text-[12px] leading-snug text-destructive">
          {rowError}
        </p>
      )}
    </div>
  );
}

/* -------------------------------------------------------------------------- */
/* Mouse capture                                                              */
/* -------------------------------------------------------------------------- */

function MouseCapture({
  current,
  onCapture,
  onCancel,
}: {
  current: Binding & { kind: "mouse" } | null;
  onCapture: (button: number) => void | Promise<void>;
  onCancel: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Focus the capture surface so Escape can cancel immediately.
    ref.current?.focus();
  }, []);

  return (
    <div
      ref={ref}
      role="button"
      tabIndex={0}
      onMouseDown={(e) => {
        e.preventDefault();
        e.stopPropagation();
        void onCapture(browserButtonToAppKit(e.button));
      }}
      onContextMenu={(e) => e.preventDefault()}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          onCancel();
        }
      }}
      className="flex min-h-[52px] cursor-pointer flex-col items-center justify-center gap-1 rounded-lg border-2 border-dashed border-[#00C7BE]/50 bg-[#00C7BE]/5 px-3 py-2 text-center outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
      aria-label="Click any mouse button here to bind it"
    >
      <span className="text-[12px] font-medium text-foreground">
        Click any mouse button here
      </span>
      <span className="text-[11px] text-muted-foreground">
        {current
          ? `Current: ${mouseLabel(current.button)} — press Esc to cancel`
          : "Left, right, middle, or a side button. Press Esc to cancel."}
      </span>
    </div>
  );
}
