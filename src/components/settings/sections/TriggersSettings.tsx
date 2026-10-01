import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Command,
  Globe,
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

import { COMMANDS, EVENTS } from "@/lib/constants.generated";
import { useEventListener } from "@/hooks/useEventListener";

import { SettingsSectionProps } from "../types";
import { SettingsGroup } from "../ui";
import { ShortcutRecorder } from "../ShortcutRecorder";
import { KeyCaps } from "../KeyCaps";

/* -------------------------------------------------------------------------- */
/* Backend contract (frozen — mirror the serde shape exactly)                 */
/* -------------------------------------------------------------------------- */

// Five independent gestures. A double tap is not a second way to reach a hold
// trigger; it is a row of its own, with its own key and its own target.
type Gesture = "hold" | "tap" | "double_tap" | "double_tap_hold" | "say";
type TriggerTarget = "agent" | "dictation";

type BindingTab = "keyboard" | "mouse";

// Two kinds, because from where the person sits there are two: a key and a
// mouse button. Fn is a key, so it is a keyboard binding whose shortcut string
// is "Fn". That it has to be watched by a native monitor rather than the
// global-shortcut plugin is the backend's business, not this screen's.
type Binding =
  | { kind: "keyboard"; shortcut: string }
  | { kind: "mouse"; button: number };

interface Trigger {
  /** The row's identity. A new row sends "" and the backend gives it one. */
  id: string;
  gesture: Gesture;
  target: TriggerTarget;
  binding: Binding | null;
  phrase: string | null;
  require_hey_prefix: boolean;
  enabled: boolean;
}

/* -------------------------------------------------------------------------- */
/* Labels + helpers                                                           */
/* -------------------------------------------------------------------------- */

// A trigger reads as one sentence about the gesture and what it does:
// "Hold to dictate", "Double-tap and hold to talk to Juno". Never the
// mechanism. Mirrors `Gesture::label` / `TriggerTarget::label` in Rust.
const GESTURE_LABEL: Record<Gesture, string> = {
  hold: "Hold",
  tap: "Tap",
  double_tap: "Double-tap",
  double_tap_hold: "Double-tap and hold",
  say: "Say",
};

const TARGET_LABEL: Record<TriggerTarget, string> = {
  agent: "to talk to Juno",
  dictation: "to dictate",
};

// How each gesture ends, mirroring `Gesture::ending` in Rust. Generated from
// the gesture alone, so the line beside a row can only ever describe that row.
// The paragraph this replaced taught a second gesture reaching the same target
// ("Double-tap it to keep listening"), which is the behaviour the gesture model
// deleted: a double tap is its own row now.
const GESTURE_ENDING: Record<Gesture, string> = {
  hold: "Let go to finish.",
  tap: "Press the key again to finish.",
  double_tap: "Press the key again to finish.",
  double_tap_hold: "Let go to finish.",
  say: "Juno starts listening when it hears the phrase.",
};

const ALL_GESTURES: Gesture[] = [
  "hold",
  "tap",
  "double_tap",
  "double_tap_hold",
  "say",
];
const ALL_TARGETS: TriggerTarget[] = ["agent", "dictation"];

const comboLabel = (gesture: Gesture, target: TriggerTarget) =>
  `${GESTURE_LABEL[gesture]} ${TARGET_LABEL[target]}`;

const errStr = (e: unknown) =>
  typeof e === "string" ? e : e instanceof Error ? e.message : String(e);

/**
 * `applyTriggers` rethrows so the recorder, which awaits it, can show the
 * reason inline. A fire-and-forget caller already has the message in
 * `rowError`, so it swallows the rejection rather than leaving an unhandled one
 * behind.
 */
const ignoreHandled = () => {};

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

/**
 * Every spelling of the globe key a shortcut string may use. Apple has called
 * the same physical key both Fn and Globe; the backend accepts either.
 */
const FN_ALIASES = ["fn", "globe"];

/** The shortcut string the backend records a globe-key press as. */
const FN_SHORTCUT = "Fn";

const isFnShortcut = (shortcut: string) =>
  FN_ALIASES.includes(shortcut.trim().toLowerCase());

/** Whether this binding is the globe key, which macOS has its own plans for. */
function isFnBinding(binding: Binding | null): boolean {
  return binding?.kind === "keyboard" && isFnShortcut(binding.shortcut);
}

function bindingLabel(binding: Binding | null): string {
  if (!binding) return "Set binding";
  if (binding.kind === "keyboard") {
    if (isFnShortcut(binding.shortcut)) return "Fn (globe)";
    return binding.shortcut || "Set binding";
  }
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

/**
 * The factory bindings, mirroring `triggers::default_triggers` in Rust: hold
 * the globe key to talk to Juno, hold Option+Space to dictate. A row whose
 * sentence is not one of those two has nothing to reset to.
 */
const DEFAULT_BINDINGS: Partial<Record<string, string>> = {
  "hold:agent": "Fn",
  "hold:dictation": "Option+Space",
};

/** A new row. The id is the backend's to hand out. */
function defaultsFor(gesture: Gesture, target: TriggerTarget): Trigger {
  return {
    id: "",
    gesture,
    target,
    binding: null,
    phrase:
      gesture === "say" ? (target === "agent" ? "juno" : "transcribe") : null,
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

  // The refusal from the last save the backend would not take, beside the row
  // that caused it.
  //
  // It only ever describes *that* attempt. An accepted save means the stored
  // list has no conflict in it, so every conflict message must go at that
  // moment, which is the fix for the error that outlived its cause: this used
  // to clear only when the row being edited happened to be the row the message
  // was pinned to, so "Fn (globe) is bound to more than one trigger" sat on
  // screen for the rest of the session once the conflict was fixed from the
  // other row.
  const [rowError, setRowError] = useState<{ id: string; message: string } | null>(
    null,
  );

  // Which key row has its binding editor open, and which capture tab it shows.
  const [editingKey, setEditingKey] = useState<string | null>(null);
  const [bindingTab, setBindingTab] = useState<Record<string, BindingTab>>(
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
    // Whatever the backend hands back is a list it accepted, so no refusal
    // about it can still be true.
    setRowError(null);
    try {
      const list = await invoke<Trigger[]>(COMMANDS.TRIGGERS_GET_TRIGGERS);
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
   * On success we adopt the backend's normalized list and drop any standing
   * refusal, because the list the backend just accepted has no conflict in it.
   * On a refusal we revert to the last accepted list and show the reason
   * beside `editedId`, then rethrow so callers (the recorder) can show it too.
   */
  const applyTriggers = useCallback(
    async (next: Trigger[], editedId: string) => {
      // Drop any save still waiting out its debounce. It carries a list from
      // before this change, so letting it land afterwards would quietly undo
      // the binding that was just recorded.
      if (saveTimer.current) {
        clearTimeout(saveTimer.current);
        saveTimer.current = null;
      }
      try {
        const normalized = await invoke<Trigger[]>(COMMANDS.TRIGGERS_SET_TRIGGERS, {
          triggers: next,
        });
        persistedRef.current = normalized;
        setTriggers(normalized);
        setRowError(null);
      } catch (e) {
        setTriggers(persistedRef.current);
        setRowError({ id: editedId, message: errStr(e) });
        throw e;
      }
    },
    [],
  );

  const scheduleSave = useCallback(
    (next: Trigger[], editedId: string) => {
      if (saveTimer.current) clearTimeout(saveTimer.current);
      saveTimer.current = setTimeout(() => {
        applyTriggers(next, editedId).catch(ignoreHandled);
      }, 300);
    },
    [applyTriggers],
  );

  /**
   * Record a binding on one row. The single path for it, so a key captured by
   * pressing it and one recorded in the editor behave identically. Whether the
   * row switches itself on is the backend's call, not this screen's.
   */
  const setBindingFor = useCallback(
    async (id: string, binding: Binding | null) => {
      const next = triggersRef.current.map((t) =>
        t.id === id ? { ...t, binding } : t,
      );
      setTriggers(next);
      await applyTriggers(next, id);
    },
    [applyTriggers],
  );

  // While a key row's editor is open on the keyboard tab, ask the backend to
  // report a bare modifier press instead of firing it. Fn never reaches a web
  // page, so without this the recorder would sit there seeing nothing while
  // the person pressed the key they wanted, and dictation would start instead.
  // The request is a lease with its own expiry on the backend, so a settings
  // window that is closed mid-capture cannot leave the key swallowed.
  const capturing = editingKey !== null && (bindingTab[editingKey] ?? "keyboard") === "keyboard";

  useEffect(() => {
    if (!capturing) return;
    void invoke(COMMANDS.TRIGGERS_SET_TRIGGER_CAPTURE, { active: true }).catch(
      (e) => console.debug("[Triggers] could not listen for the globe key:", e),
    );
    return () => {
      void invoke(COMMANDS.TRIGGERS_SET_TRIGGER_CAPTURE, {
        active: false,
      }).catch((e) =>
        console.debug("[Triggers] could not stop listening for the globe key:", e),
      );
    };
  }, [capturing]);

  useEventListener<{ key?: string; shortcut?: string }>(
    EVENTS.TRIGGERS_KEY_CAPTURED,
    (payload) => {
      // Only the row that asked for it, and only while it is still asking.
      if (!capturing || !editingKey) return;
      const shortcut = payload?.shortcut || FN_SHORTCUT;
      setEditingKey(null);
      void setBindingFor(editingKey, { kind: "keyboard", shortcut });
    },
  );

  const patchTrigger = useCallback(
    (id: string, patch: Partial<Trigger>, opts: { immediate?: boolean } = {}) => {
      const next = triggersRef.current.map((t) =>
        t.id === id ? { ...t, ...patch } : t,
      );
      setTriggers(next);
      if (opts.immediate) applyTriggers(next, id).catch(ignoreHandled);
      else scheduleSave(next, id);
    },
    [applyTriggers, scheduleSave],
  );

  const addTrigger = useCallback(
    async (gesture: Gesture, target: TriggerTarget) => {
      const created = defaultsFor(gesture, target);
      const next = [...triggers, created];
      setTriggers(next);
      // The id comes back with the saved list, so the record flow waits for it:
      // a row cannot be edited before it has an identity to be edited by.
      try {
        const saved = await invoke<Trigger[]>(COMMANDS.TRIGGERS_SET_TRIGGERS, {
          triggers: next,
        });
        persistedRef.current = saved;
        setTriggers(saved);
        setRowError(null);
        if (gesture === "say") return; // a Say row edits its phrase in place
        const added = saved[saved.length - 1];
        if (!added) return;
        setBindingTab((prev) => ({ ...prev, [added.id]: "keyboard" }));
        setEditingKey(added.id);
      } catch (e) {
        setTriggers(persistedRef.current);
        setRowError({ id: "", message: errStr(e) });
      }
    },
    [triggers],
  );

  const removeTrigger = useCallback(
    (id: string) => {
      const next = triggers.filter((t) => t.id !== id);
      setTriggers(next);
      if (editingKey === id) setEditingKey(null);
      setRowError((prev) => (prev?.id === id ? null : prev));
      applyTriggers(next, id).catch(ignoreHandled);
    },
    [triggers, editingKey, applyTriggers],
  );

  // Every gesture, for every target. Nothing is filtered out: a row is its own
  // identity now, so two Hold rows on two different keys are an ordinary pair
  // rather than a duplicate. Which gestures may share one *key* is the
  // backend's table, and a refusal names the row that already has it.
  const addable: Array<{ gesture: Gesture; target: TriggerTarget }> = [];
  for (const gesture of ALL_GESTURES)
    for (const target of ALL_TARGETS) addable.push({ gesture, target });

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
        <Button size="sm">
          <Plus className="size-3.5" />
          Add trigger
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-64">
        {addable.map(({ gesture, target }) => (
          <DropdownMenuItem
            key={`${gesture}:${target}`}
            onSelect={() => void addTrigger(gesture, target)}
          >
            {gesture === "say" ? (
              <Mic className="size-3.5 text-muted-foreground" />
            ) : (
              <Keyboard className="size-3.5 text-muted-foreground" />
            )}
            {comboLabel(gesture, target)}
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
                A trigger is how you summon Juno: a key, a mouse button, or
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
        footer="Each row is one way to summon Juno. Mix keys, a mouse button and your voice however you like."
      >
        {triggers.map((trigger) => (
          <TriggerRow
            key={trigger.id}
            trigger={trigger}
            editing={editingKey === trigger.id}
            bindingTab={bindingTab[trigger.id] ?? "keyboard"}
            rowError={rowError?.id === trigger.id ? rowError.message : null}
            alwaysListening={Boolean(settings.alwaysListeningActive)}
            onOpenEditor={() => {
              setBindingTab((prev) => ({
                ...prev,
                [trigger.id]:
                  trigger.binding?.kind === "mouse" ? "mouse" : "keyboard",
              }));
              setEditingKey(trigger.id);
            }}
            onCloseEditor={() => setEditingKey(null)}
            onTabChange={(tab) =>
              setBindingTab((prev) => ({ ...prev, [trigger.id]: tab }))
            }
            onPatch={patchTrigger}
            onSetBinding={setBindingFor}
            onRemove={() => removeTrigger(trigger.id)}
          />
        ))}
      </SettingsGroup>

      {/* A refusal the list as a whole caused, such as a key a row that has
          since been deleted was holding. Shown here rather than nowhere. */}
      {rowError && !triggers.some((t) => t.id === rowError.id) && (
        <p className="px-1 text-[12px] leading-snug text-destructive" role="alert">
          {rowError.message}
        </p>
      )}

      <div className="flex items-center justify-end px-1">{addMenu}</div>
    </div>
  );
}

/* -------------------------------------------------------------------------- */
/* Row                                                                        */
/* -------------------------------------------------------------------------- */

interface TriggerRowProps {
  trigger: Trigger;
  editing: boolean;
  bindingTab: BindingTab;
  rowError: string | null;
  alwaysListening: boolean;
  onOpenEditor: () => void;
  onCloseEditor: () => void;
  onTabChange: (tab: BindingTab) => void;
  onPatch: (
    id: string,
    patch: Partial<Trigger>,
    opts?: { immediate?: boolean },
  ) => void;
  /** Record a binding on this row. One implementation, in the parent, so a
      key recorded by pressing it and one chosen here take the same path. */
  onSetBinding: (id: string, binding: Binding | null) => Promise<void>;
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
  onSetBinding,
  onRemove,
}: TriggerRowProps) {
  const key = trigger.id;
  const isVoice = trigger.gesture === "say";

  const GestureIcon = isVoice
    ? Mic
    : trigger.binding?.kind === "mouse"
      ? MousePointer2
      : trigger.gesture === "tap" || trigger.gesture === "double_tap"
        ? Command
        : Keyboard;

  const setBindingNow = (binding: Binding | null) => onSetBinding(key, binding);

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
          <GestureIcon className="h-3.5 w-3.5" />
        </span>

        <div className={cn("min-w-0 flex-1", !trigger.enabled && "opacity-55")}>
          {/* One sentence: the gesture, then what it does. */}
          <div className="flex items-baseline gap-1 text-[13px]">
            <span className="font-medium">{GESTURE_LABEL[trigger.gesture]}</span>
            <span className="text-muted-foreground">{TARGET_LABEL[trigger.target]}</span>
          </div>
          <p className="text-[12px] leading-snug text-muted-foreground">
            {GESTURE_ENDING[trigger.gesture]}
          </p>
        </div>

        {/* Binding chip (key methods only) */}
        {!isVoice && (
          <Button
            variant="outline"
            size="xs"
            onClick={() => (editing ? onCloseEditor() : onOpenEditor())}
            aria-label={`Edit binding for ${comboLabel(
              trigger.gesture,
              trigger.target,
            )}`}
            className={cn(
              "font-mono",
              !trigger.binding && "text-muted-foreground",
            )}
          >
            {trigger.binding?.kind === "mouse" ? (
              <>
                <MousePointer2 className="size-3" />
                {bindingLabel(trigger.binding)}
              </>
            ) : trigger.binding?.kind === "keyboard" && trigger.binding.shortcut ? (
              <KeyCaps shortcut={trigger.binding.shortcut} />
            ) : (
              <>
                <Keyboard className="size-3" />
                {bindingLabel(trigger.binding)}
              </>
            )}
          </Button>
        )}

        <Switch
          checked={trigger.enabled}
          onCheckedChange={(checked) =>
            onPatch(key, { enabled: checked }, { immediate: true })
          }
          aria-label={`Enable ${comboLabel(trigger.gesture, trigger.target)}`}
        />

        <Button
          variant="ghost"
          size="icon-xs"
          onClick={onRemove}
          aria-label={`Delete ${comboLabel(trigger.gesture, trigger.target)}`}
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
            <div className="space-y-2">
              <ShortcutRecorder
                value={
                  trigger.binding?.kind === "keyboard"
                    ? trigger.binding.shortcut
                    : ""
                }
                shortcutName={`trigger_${key}`}
                defaultShortcut={
                  DEFAULT_BINDINGS[`${trigger.gesture}:${trigger.target}`] ??
                  null
                }
                onSave={(shortcut) =>
                  setBindingNow({ kind: "keyboard", shortcut })
                }
                onCancel={onCloseEditor}
              />
              {/* The globe key is a key, so it is recorded by pressing it like
                  any other. It just never reaches this page, so the press is
                  reported by the backend while this editor is open. */}
              <p className="flex items-center gap-1.5 text-[12px] text-muted-foreground">
                <Globe className="size-3 shrink-0" aria-hidden />
                Or press the globe key (Fn) now to use that.
              </p>
            </div>
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

      {/* Only beside an Fn binding: macOS has already given that key a job,
          and nothing else on this screen is affected by it. */}
      {!isVoice && isFnBinding(trigger.binding) && <GlobeKeyNote />}

      {rowError && (
        <p className="mt-2 text-[12px] leading-snug text-destructive">
          {rowError}
        </p>
      )}
    </div>
  );
}

/* -------------------------------------------------------------------------- */
/* The globe key note                                                         */
/* -------------------------------------------------------------------------- */

/**
 * What macOS does with the globe key before Juno gets a say.
 *
 * Shown only where it applies, next to a trigger bound to Fn, because it is
 * advice about one key and not a standing notice about triggers. The link goes
 * to the Keyboard pane rather than describing a path through System Settings;
 * macOS has no deeper link than the pane, so the sentence still names the row.
 */
function GlobeKeyNote() {
  const [failed, setFailed] = useState(false);

  return (
    <p className="mt-2 flex items-start gap-1.5 text-[12px] leading-snug text-muted-foreground">
      <Globe className="mt-[2px] size-3 shrink-0" aria-hidden />
      <span>
        macOS gives the globe key its own job by default. Set{" "}
        <button
          type="button"
          onClick={async () => {
            try {
              await invoke(COMMANDS.TRIGGERS_OPEN_KEYBOARD_SETTINGS);
              setFailed(false);
            } catch (e) {
              console.debug("[Triggers] could not open Keyboard settings:", e);
              setFailed(true);
            }
          }}
          className="text-[#007AFF] underline-offset-2 hover:underline"
        >
          System Settings, Keyboard
        </button>
        , "Press globe key to" to "Do Nothing", or it will open the emoji picker
        every time you talk to Juno.
        {failed && (
          <span className="block text-muted-foreground/80">
            Juno could not open that pane. Open System Settings and look under
            Keyboard.
          </span>
        )}
      </span>
    </p>
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
          ? `Current: ${mouseLabel(current.button)}. Press Esc to cancel.`
          : "Left, right, middle, or a side button. Press Esc to cancel."}
      </span>
    </div>
  );
}
