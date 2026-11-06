import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import {
  AlertCircle,
  ChevronDown,
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
import { Toggle } from "@/components/ui/toggle";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";

import { COMMANDS, EVENTS } from "@/lib/constants.generated";
import { useEventListener } from "@/hooks/useEventListener";
import { useDerivedIssues } from "@/hooks/useDerivedIssues";

import { SettingsSectionProps } from "../types";
import { SETTINGS_ROW_ID_PREFIX, SettingsGroup } from "../ui";
import { useAdvancedSettings } from "../AdvancedSettingsContext";
import { ShortcutRecorder } from "../ShortcutRecorder";
import { KeyCaps } from "../KeyCaps";

/* -------------------------------------------------------------------------- */
/* Backend contract (frozen — mirror the serde shape exactly)                 */
/* -------------------------------------------------------------------------- */

// Three independent gestures, each a row of its own with its own key and its
// own target.
type Gesture = "hold" | "tap" | "say";
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
  /** Set by the backend when it moved this row off the globe key because no
   * connected keyboard has one. Sent back untouched; never shown. */
  keyboard_fallback?: boolean;
}

/**
 * One thing wrong with the list now, addressed to the row that causes it.
 * Mirrors `triggers::TriggerIssue` in Rust, which derives the whole set from
 * the list every time it is asked, so there is no such thing as an old issue.
 */
interface TriggerIssue {
  trigger_id: string;
  message: string;
}

/* -------------------------------------------------------------------------- */
/* Labels + helpers                                                           */
/* -------------------------------------------------------------------------- */

// A trigger reads as one sentence about the gesture and what it does:
// "Hold to dictate", "Tap to talk to Juno". Never the
// mechanism. Mirrors `Gesture::label` / `TriggerTarget::label` in Rust.
const GESTURE_LABEL: Record<Gesture, string> = {
  hold: "Hold",
  tap: "Tap",
  say: "Say",
};

const TARGET_LABEL: Record<TriggerTarget, string> = {
  agent: "to talk to Juno",
  dictation: "to dictate",
};

// How each gesture ends, mirroring `Gesture::ending` in Rust. Generated from
// the gesture alone, so the line beside a row can only ever describe that row.
const GESTURE_ENDING: Record<Gesture, string> = {
  hold: "Let go to finish.",
  tap: "Press the key again to finish.",
  say: "Juno starts listening when it hears the phrase.",
};

const ALL_GESTURES: Gesture[] = ["hold", "tap", "say"];

/**
 * The gestures a key or mouse row can have. A wake phrase is left out on
 * purpose: it is a different kind of row, with a phrase where the key goes,
 * so offering it as another gesture would quietly throw a recorded key away.
 */
const KEY_GESTURES: Gesture[] = ALL_GESTURES.filter((g) => g !== "say");
const ALL_TARGETS: TriggerTarget[] = ["agent", "dictation"];

const comboLabel = (gesture: Gesture, target: TriggerTarget) =>
  `${GESTURE_LABEL[gesture]} ${TARGET_LABEL[target]}`;

/**
 * The target as the bare thing it does. The sentence on screen supplies the
 * "to" itself, so the word somebody picks from a menu is "dictate", not "to
 * dictate". Derived from the Rust mirror rather than written out again, so the
 * two cannot drift apart.
 */
const targetWord = (target: TriggerTarget) =>
  TARGET_LABEL[target].replace(/^to /, "");

/**
 * The whole row read aloud, key included: "Hold Fn (globe) to talk to Juno".
 * It is the row's accessible name, so a screen reader hears the sentence
 * somebody with eyes reads rather than a pile of unlabelled controls.
 */
const sentenceLabel = (trigger: Trigger) =>
  trigger.gesture === "say"
    ? `Say "${trigger.require_hey_prefix ? "hey " : ""}${trigger.phrase || "…"}" ${TARGET_LABEL[trigger.target]}`
    : `${GESTURE_LABEL[trigger.gesture]} ${bindingLabel(trigger.binding)} ${TARGET_LABEL[trigger.target]}`;

const errStr = (e: unknown) =>
  typeof e === "string" ? e : e instanceof Error ? e.message : String(e);

/**
 * Ask Rust what is wrong with this list. Module scope, so it is one stable
 * function and `useDerivedIssues` is not re-asking because the component
 * re-rendered.
 *
 * The rule, the sharing table and the wording all live in `triggers::issues`,
 * which is also what a save refuses on. This screen renders that answer and
 * owns none of it, so a conflict reads the same sentence wherever it is shown
 * and goes away for the same reasons everywhere.
 */
const listIssues = (list: Trigger[]) =>
  invoke<TriggerIssue[]>(COMMANDS.TRIGGERS_GET_TRIGGER_ISSUES, {
    triggers: list,
  });

/**
 * `applyTriggers` rethrows so the recorder, which awaits it, can show the
 * reason inline. A fire-and-forget caller needs nothing from the rejection,
 * because the sentence it would have carried is derived from the list that is
 * now on screen, so it swallows it rather than leaving an unhandled one behind.
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

/** The shortcut string the backend records a globe-key press as. */
const FN_SHORTCUT = "Fn";

/**
 * Each modifier a modifier-only binding can hold, in the order the backend
 * writes them. Mirrors `ModifierKey` in Rust: Apple has called the globe key
 * both Fn and Globe, and the backend accepts either.
 */
const MODIFIER_WORDS: Array<{ name: string; words: string[] }> = [
  { name: "Fn", words: ["fn", "globe"] },
  { name: "Control", words: ["control", "ctrl"] },
  { name: "Option", words: ["option", "opt", "alt"] },
  { name: "Shift", words: ["shift"] },
  { name: "Command", words: ["command", "cmd", "meta", "super"] },
];

const RIGHT_OPTION_WORDS = [
  "rightoption",
  "right option",
  "right_option",
  "rightalt",
  "right alt",
  "roption",
];

/**
 * The modifiers a modifier-only shortcut names, in canonical order, or null
 * when it names an ordinary key too. Right Option alone is ["Right Option"].
 */
function modifierSet(shortcut: string): string[] | null {
  const parts = shortcut.split("+").map((p) => p.trim().toLowerCase());
  if (parts.length === 1 && RIGHT_OPTION_WORDS.includes(parts[0])) return ["Right Option"];
  const names: string[] = [];
  for (const part of parts) {
    const found = MODIFIER_WORDS.find((m) => m.words.includes(part))
      ?? (RIGHT_OPTION_WORDS.includes(part) ? MODIFIER_WORDS[2] : undefined);
    if (!found || names.includes(found.name)) return null;
    names.push(found.name);
  }
  return MODIFIER_WORDS.map((m) => m.name).filter((n) => names.includes(n));
}

/** Whether this binding uses the globe key, which macOS has its own plans for. */
function isFnBinding(binding: Binding | null): boolean {
  return (
    binding?.kind === "keyboard" &&
    (modifierSet(binding.shortcut)?.includes(FN_SHORTCUT) ?? false)
  );
}

function bindingLabel(binding: Binding | null): string {
  if (!binding) return "Set binding";
  if (binding.kind === "keyboard") {
    // A modifier-only binding reads the way the backend names it in a
    // conflict, so the row and the sentence beside it agree.
    const set = modifierSet(binding.shortcut);
    if (set && set.length === 1 && set[0] === FN_SHORTCUT) return "Fn (globe)";
    if (set) return set.join(" + ");
    return binding.shortcut || "Set binding";
  }
  return mouseLabel(binding.button);
}

/**
 * The keys that can only ever be held, never tapped. Mirrors
 * `ModifierKey::hold_only` in Rust: Control and the right-hand Option key are
 * ordinary shortcut ingredients, so counting a tap on one would fire Juno on
 * every Control-click. A chord may be tapped, because the monitor only counts
 * a tap whose keys came up with nothing else pressed in between.
 *
 * `triggers_hold_only_matches_rust.test.ts` reads the Rust source and fails if
 * these two lists stop agreeing.
 */
const HOLD_ONLY_MODIFIERS = ["Control", "Right Option"];

/**
 * Why this binding cannot be tapped, or null when it can be.
 *
 * The sentence is the one `triggers::issues` refuses the save with, so the
 * greyed-out gesture and the refusal it is preventing read identically. Nobody
 * should be able to pick Tap and then be told no: the menu says no first.
 */
function holdOnlyReason(binding: Binding | null): string | null {
  if (binding?.kind !== "keyboard") return null;
  const set = modifierSet(binding.shortcut);
  if (!set || set.length !== 1 || !HOLD_ONLY_MODIFIERS.includes(set[0])) {
    return null;
  }
  return `"${bindingLabel(binding)}" can only be held, not tapped.`;
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
 * the globe key to talk to Juno, hold the globe key and Control to dictate. A
 * row whose sentence is not one of those two has nothing to reset to.
 */
const DEFAULT_BINDINGS: Partial<Record<string, string>> = {
  "hold:agent": "Fn",
  "hold:dictation": "Fn+Control",
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
  // Wake phrases are experimental for launch. Outside advanced settings a Say
  // row is not offered and not drawn, and Rust does not arm it either, so
  // nobody is shown a voice option that would not reliably answer. Hidden
  // rows stay in `triggers`, so saving the list never deletes them.
  const { advanced } = useAdvancedSettings();
  const [triggers, setTriggers] = useState<Trigger[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);

  // What is wrong with the list on screen, asked of Rust every time the list
  // changes and never remembered between answers.
  //
  // This replaced a refusal held in local state, which is why the message used
  // to outlive its cause. A rejected save handed down one sentence pinned to
  // the row being edited, and that sentence then needed something to come along
  // and invalidate it. An accepted save did; nothing else did, so somebody who
  // hit the conflict and simply stopped was left reading "Fn (globe) already
  // has Hold to talk to Juno" for the rest of the session, about a list that no
  // longer looked like that. There is nowhere for a stale one to live now: the
  // question is asked about the rows being drawn, so the answer cannot describe
  // anything else, and it is never hidden on a timer while it is still true.
  const issues = useDerivedIssues(triggers, listIssues, !loading);
  const issueFor = (id: string) =>
    issues.find((i) => i.trigger_id === id)?.message ?? null;

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
   *
   * A refused save leaves the list as the person left it, which is the other
   * half of deriving the message. Reverting used to throw their edit away and
   * then show a sentence about a conflict that was no longer anywhere on
   * screen, so the message had to be remembered, and remembering it is what let
   * it go stale. Keeping the edit makes the sentence a true statement about a
   * visible row, lets it clear the moment they fix it from either side, and
   * means fixing the other row saves the binding they wanted instead of making
   * them record it again. The invalid list is only ever in this window: the
   * backend refused it, so no key is registered to it.
   *
   * A failure the derivation does not explain is a different thing. It
   * describes an attempt, not a field, so it reverts and goes to a toast.
   */
  const applyTriggers = useCallback(
    async (next: Trigger[]) => {
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
      } catch (e) {
        const explained = await listIssues(next).catch(() => []);
        if (explained.length === 0) {
          setTriggers(persistedRef.current);
          toast.error(errStr(e));
        }
        throw e;
      }
    },
    [],
  );

  const scheduleSave = useCallback(
    (next: Trigger[]) => {
      if (saveTimer.current) clearTimeout(saveTimer.current);
      saveTimer.current = setTimeout(() => {
        applyTriggers(next).catch(ignoreHandled);
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
      await applyTriggers(next);
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

  // Whether "Press globe key to" still needs changing, asked of the system.
  // Unknown or failed reads show nothing: advice that may not apply is worse
  // than none.
  const [globeNeedsSetup, setGlobeNeedsSetup] = useState(false);
  useEffect(() => {
    let cancelled = false;
    invoke<boolean>(COMMANDS.TRIGGERS_GLOBE_KEY_NEEDS_SETUP)
      .then((needs) => {
        if (!cancelled) setGlobeNeedsSetup(needs === true);
      })
      .catch((e) => console.debug("[Triggers] could not read the globe key setting:", e));
    return () => {
      cancelled = true;
    };
  }, []);

  // The backend moves the default holds on its own when a keyboard with or
  // without a globe key comes or goes. Read the list again so the keys on
  // screen are the keys that work.
  useEventListener(EVENTS.TRIGGERS_CHANGED, () => {
    void load();
  });

  const patchTrigger = useCallback(
    (id: string, patch: Partial<Trigger>, opts: { immediate?: boolean } = {}) => {
      const next = triggersRef.current.map((t) =>
        t.id === id ? { ...t, ...patch } : t,
      );
      setTriggers(next);
      if (opts.immediate) applyTriggers(next).catch(ignoreHandled);
      else scheduleSave(next);
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
        if (gesture === "say") return; // a Say row edits its phrase in place
        const added = saved[saved.length - 1];
        if (!added) return;
        setBindingTab((prev) => ({ ...prev, [added.id]: "keyboard" }));
        setEditingKey(added.id);
      } catch (e) {
        // A new row is blank, so it cannot be the thing a rule refuses. Any
        // refusal here is about the save itself.
        setTriggers(persistedRef.current);
        toast.error(errStr(e));
      }
    },
    [triggers],
  );

  const removeTrigger = useCallback(
    (id: string) => {
      const next = triggers.filter((t) => t.id !== id);
      setTriggers(next);
      if (editingKey === id) setEditingKey(null);
      applyTriggers(next).catch(ignoreHandled);
    },
    [triggers, editingKey, applyTriggers],
  );

  // Adding is one press, not a choice between six. This used to be a menu of
  // every gesture crossed with every target, because a row's words could not
  // be changed once it existed: picking the wrong one meant deleting the row
  // and starting again, so the menu had to be right first time. Now every word
  // in a sentence is editable where it is read, so the button adds the next
  // sentence nobody has yet and opens the recorder on it, and anything about
  // it that is wrong is a word away from being right.
  const taken = new Set(triggers.map((t) => `${t.gesture}:${t.target}`));
  const nextSentence = (() => {
    for (const gesture of KEY_GESTURES)
      for (const target of ALL_TARGETS)
        if (!taken.has(`${gesture}:${target}`)) return { gesture, target };
    return { gesture: "hold" as Gesture, target: "agent" as TriggerTarget };
  })();

  // A wake phrase is a different kind of row, not another gesture, so it is
  // added separately and only where it is offered at all. One per target,
  // because two rows cannot hold the same phrase and a blank one cannot save.
  const nextSayTarget = advanced
    ? (ALL_TARGETS.find((target) => !taken.has(`say:${target}`)) ?? null)
    : null;

  const visibleTriggers = advanced
    ? triggers
    : triggers.filter((t) => t.gesture !== "say");

  /* --------------------------- loading / error --------------------------- */

  if (loading) {
    return (
      <div className="space-y-6">
        <SettingsGroup title="Shortcuts">
          <div className="flex items-center justify-center py-10 text-muted-foreground">
            <RefreshCw className="h-5 w-5 animate-spin" />
            <span className="ml-2 text-[13px]">Loading shortcuts…</span>
          </div>
        </SettingsGroup>
      </div>
    );
  }

  if (loadError) {
    return (
      <div className="space-y-6">
        <SettingsGroup title="Shortcuts">
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

  const addButtons = (
    <>
      {nextSayTarget && (
        <Button
          variant="ghost"
          size="sm"
          onClick={() => void addTrigger("say", nextSayTarget)}
        >
          <Mic className="size-3.5" />
          Add wake phrase
        </Button>
      )}
      <Button
        size="sm"
        onClick={() =>
          void addTrigger(nextSentence.gesture, nextSentence.target)
        }
      >
        <Plus className="size-3.5" />
        Add shortcut
      </Button>
    </>
  );

  /* ------------------------------ empty state ---------------------------- */

  if (visibleTriggers.length === 0) {
    return (
      <div className="space-y-6">
        <SettingsGroup title="Shortcuts">
          <div className="flex flex-col items-center gap-4 px-6 py-12 text-center">
            <span className="flex h-12 w-12 items-center justify-center rounded-2xl bg-[#00C7BE]/15 text-[#00C7BE]">
              <Zap className="h-6 w-6" />
            </span>
            <div className="space-y-1">
              <p className="text-[14px] font-semibold">No shortcuts yet</p>
              <p className="text-[12px] leading-snug text-muted-foreground">
                {advanced
                  ? "A shortcut is how you summon Juno: a key, a mouse button, or your voice."
                  : "A shortcut is how you summon Juno: a key or a mouse button."}
              </p>
            </div>
            <div className="flex flex-wrap items-center justify-center gap-2">
              {addButtons}
            </div>
          </div>
        </SettingsGroup>
      </div>
    );
  }

  /* ------------------------------- list ---------------------------------- */

  const anchors = triggerAnchors(triggers);

  const usesFn = visibleTriggers.some(
    (t) => t.enabled && isFnBinding(t.binding),
  );

  return (
    <div className="space-y-6">
      <SettingsGroup
        title="Shortcuts"
        footer={
          advanced
            ? "Each row is one way to summon Juno. Mix keys, a mouse button and your voice however you like."
            : "Each row is one way to summon Juno. Mix keys and a mouse button however you like."
        }
      >
        {visibleTriggers.map((trigger) => (
          <TriggerSentence
            key={trigger.id}
            anchor={anchors.get(trigger.id)}
            trigger={trigger}
            editing={editingKey === trigger.id}
            bindingTab={bindingTab[trigger.id] ?? "keyboard"}
            issue={issueFor(trigger.id)}
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

      {/* An issue addressed to a row this list is not drawing, which only a
          row the window has not saved yet can produce. Shown here rather than
          nowhere, and it goes when the derivation stops returning it. */}
      {issues
        .filter((i) => !triggers.some((t) => t.id === i.trigger_id))
        .map((i) => (
          <IssueLine
            key={`${i.trigger_id}:${i.message}`}
            message={i.message}
            className="px-1"
          />
        ))}

      {/* Said once for the whole screen, and only while it is true: macOS
          gives the globe key a job of its own unless the setting says
          otherwise, and a key that already does nothing needs no advice. */}
      {usesFn && globeNeedsSetup && <GlobeKeyNote />}

      <div className="flex items-center justify-end gap-2 px-1">
        {addButtons}
      </div>
    </div>
  );
}

/* -------------------------------------------------------------------------- */
/* One editable word                                                          */
/* -------------------------------------------------------------------------- */

interface WordOption<T extends string> {
  value: T;
  label: string;
  /**
   * Why this word cannot be chosen, from the same rule the save refuses on,
   * or null when it can be. Shown under the greyed word, where the choice is.
   */
  blocked?: string | null;
}

/**
 * One word in a sentence that can be swapped for another.
 *
 * It reads as the word it stands for, not as a form control: no border, no
 * background until the pointer is on it. The chevron is the whole affordance,
 * because the sentence is the thing being read and a row of outlined selects
 * would stop it being a sentence.
 *
 * A blocked word stays focusable (`aria-disabled` rather than `disabled`,
 * which Radix skips over) so somebody arrowing through the menu hears why it
 * cannot be picked instead of finding the option missing.
 */
function SentenceWord<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
}: {
  value: T;
  options: Array<WordOption<T>>;
  onChange: (next: T) => void;
  ariaLabel: string;
}) {
  const current = options.find((o) => o.value === value);

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={`${ariaLabel} (currently ${current?.label ?? value})`}
          className={cn(
            // Never shrinks: these are three or four known words, so the
            // sentence wraps to the next line rather than eating one of them.
            "-mx-0.5 inline-flex shrink-0 items-center gap-0.5 rounded-[5px] px-1 py-px",
            "hover:bg-accent focus-visible:outline-none focus-visible:ring-[1.5px] focus-visible:ring-ring",
            "data-[state=open]:bg-accent",
          )}
        >
          <span className="whitespace-nowrap">{current?.label ?? value}</span>
          <ChevronDown className="size-3 shrink-0 text-muted-foreground/70" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-60">
        <DropdownMenuRadioGroup value={value}>
          {options.map((option) => (
            <DropdownMenuRadioItem
              key={option.value}
              value={option.value}
              aria-disabled={option.blocked ? true : undefined}
              onSelect={(e) => {
                if (option.blocked) {
                  e.preventDefault();
                  return;
                }
                onChange(option.value);
              }}
              className={cn("items-start", option.blocked && "opacity-55")}
            >
              <span className="flex min-w-0 flex-col gap-0.5">
                <span>{option.label}</span>
                {option.blocked && (
                  <span className="text-[11px] leading-snug text-muted-foreground">
                    {option.blocked}
                  </span>
                )}
              </span>
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/* -------------------------------------------------------------------------- */
/* One sentence                                                              */
/* -------------------------------------------------------------------------- */

interface TriggerSentenceProps {
  trigger: Trigger;
  editing: boolean;
  bindingTab: BindingTab;
  /** What is wrong with this row now, derived, or null while it is fine. */
  issue: string | null;
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
  /** Row anchor for search and the agent's `settings` tool, if this row has one. */
  anchor?: string;
}

/**
 * The first key or mouse row for each target is the one the agent's
 * `settings` tool calls "the shortcut" (Rust: `settings_tool::shortcut_row`),
 * so that row carries the anchor the tool highlights.
 */
const TRIGGER_ANCHOR: Record<Trigger["target"], string> = {
  agent: "trigger-agent",
  dictation: "trigger-dictation",
};

/** Anchor for each row id: the first non-voice row per target gets one. */
function triggerAnchors(triggers: Trigger[]): Map<string, string> {
  const anchors = new Map<string, string>();
  const claimed = new Set<string>();
  for (const t of triggers) {
    if (t.gesture === "say" || claimed.has(t.target)) continue;
    claimed.add(t.target);
    anchors.set(t.id, TRIGGER_ANCHOR[t.target]);
  }
  return anchors;
}

function TriggerSentence({
  anchor,
  trigger,
  editing,
  bindingTab,
  issue,
  alwaysListening,
  onOpenEditor,
  onCloseEditor,
  onTabChange,
  onPatch,
  onSetBinding,
  onRemove,
}: TriggerSentenceProps) {
  const key = trigger.id;
  const isVoice = trigger.gesture === "say";

  const setBindingNow = (binding: Binding | null) => onSetBinding(key, binding);

  // The gestures this row could have, and why it cannot have one. Tap is the
  // only gesture a key can refuse, and `holdOnlyReason` is the sentence the
  // save would refuse with, so the menu says no before anything is saved.
  const gestureOptions: Array<WordOption<Gesture>> = KEY_GESTURES.map(
    (gesture) => ({
      value: gesture,
      label: GESTURE_LABEL[gesture],
      blocked: gesture === "tap" ? holdOnlyReason(trigger.binding) : null,
    }),
  );

  // Nothing can forbid a target: no rule in `triggers::issues` reads one.
  const targetOptions: Array<WordOption<TriggerTarget>> = ALL_TARGETS.map(
    (target) => ({ value: target, label: targetWord(target) }),
  );

  return (
    <div
      id={anchor ? `${SETTINGS_ROW_ID_PREFIX}${anchor}` : undefined}
      className={cn(
        "px-4 py-3",
        anchor &&
          "scroll-mt-16 transition-colors duration-300 motion-reduce:transition-none",
      )}
    >
      {/* No icon column. Every key row carried the same keyboard glyph in a
          teal tile, which said nothing the sentence does not say better and
          repeated the on/off the switch already shows. The sentence starts
          the row. */}
      <div className="flex items-center gap-3">
        {/* One sentence, with every word in it that can change editable where
            it is read: "Hold [Fn (globe)] to [talk to Juno]". The row carries
            the whole sentence as its accessible name, so the controls inside
            it are heard in the order they are read. */}
        <div
          role="group"
          aria-label={sentenceLabel(trigger)}
          className={cn(
            "flex min-w-0 flex-1 flex-wrap items-center gap-x-1.5 gap-y-1 text-[13px]",
            !trigger.enabled && "opacity-55",
          )}
        >
          {/* Say is the only gesture a voice row can have, so it is a word
              rather than a menu of one. */}
          {isVoice ? (
            <span className="font-medium">{GESTURE_LABEL.say}</span>
          ) : (
            <span className="font-medium">
              <SentenceWord
                value={trigger.gesture}
                options={gestureOptions}
                onChange={(gesture) =>
                  onPatch(key, { gesture }, { immediate: true })
                }
                ariaLabel="Gesture"
              />
            </span>
          )}

          {isVoice ? (
            <>
              <Toggle
                variant="outline"
                size="sm"
                pressed={trigger.require_hey_prefix}
                onPressedChange={(pressed) =>
                  onPatch(key, { require_hey_prefix: pressed }, { immediate: true })
                }
                aria-label="Require the “hey” prefix"
                className="h-6 min-w-0 rounded-md px-2 font-mono text-[13px] data-[state=off]:text-muted-foreground/60"
              >
                hey
              </Toggle>
              <Input
                value={trigger.phrase ?? ""}
                onChange={(e) => onPatch(key, { phrase: e.target.value })}
                placeholder={trigger.target === "agent" ? "juno" : "transcribe"}
                aria-label="Wake phrase"
                className="h-6 w-28 px-2 font-mono text-[13px]"
              />
            </>
          ) : (
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
              ) : trigger.binding?.kind === "keyboard" &&
                trigger.binding.shortcut ? (
                <KeyCaps shortcut={trigger.binding.shortcut} />
              ) : (
                <>
                  <Keyboard className="size-3" />
                  {bindingLabel(trigger.binding)}
                </>
              )}
            </Button>
          )}

          {/* The preposition belongs to the sentence, not to the choice, so
              the menu offers "dictate" rather than "to dictate". */}
          <span className="text-muted-foreground">to</span>
          <SentenceWord
            value={trigger.target}
            options={targetOptions}
            onChange={(target) => onPatch(key, { target }, { immediate: true })}
            ariaLabel="What it does"
          />

          {/* How it ends, generated from the gesture, so this line can only
              ever describe the row it sits in. */}
          <span className="text-[12px] text-muted-foreground">
            {GESTURE_ENDING[trigger.gesture]}
          </span>
        </div>

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

      {/* What a voice row cannot say inside its sentence: whether a
          microphone is actually open, and that none of this is reliable yet.
          The phrase and the prefix are edited in the sentence above, so they
          are not repeated here. */}
      {isVoice && (
        <div
          className={cn(
            "mt-2 space-y-1",
            !trigger.enabled && "opacity-55",
          )}
        >
          <div className="flex items-center gap-2">
            {/* Only said when the sentence does not already show it: with the
                prefix off, Juno answers to the phrase either way. */}
            {!trigger.require_hey_prefix && (
              <p className="min-w-0 text-[12px] text-muted-foreground">
                <span className="font-medium text-foreground">
                  {`\u201chey ${trigger.phrase || "\u2026"}\u201d`}
                </span>{" "}
                works too.
              </p>
            )}
            <span
              className={cn(
                "ml-auto flex shrink-0 items-center gap-1.5 text-[11px]",
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
          <p className="text-[11px] leading-snug text-muted-foreground">
            Experimental. Juno can miss the phrase, and the microphone stays
            on while this row is on.
          </p>
        </div>
      )}

      {/* Binding editor (key methods, when open) */}
      {!isVoice && editing && (
        <div className="mt-3 space-y-3 rounded-md border bg-muted/30 p-2.5">
          <ToggleGroup
            type="single"
            variant="outline"
            size="sm"
            aria-label="Bind to"
            value={bindingTab}
            onValueChange={(tab) => {
              if (tab) onTabChange(tab as "keyboard" | "mouse");
            }}
          >
            <ToggleGroupItem value="keyboard" className="h-7 flex-none px-2.5 text-[12px]">
              Keyboard
            </ToggleGroupItem>
            <ToggleGroupItem value="mouse" className="h-7 flex-none px-2.5 text-[12px]">
              Mouse
            </ToggleGroupItem>
          </ToggleGroup>

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
              {/* Modifier keys on their own (the globe key, Control and
                  Option together) are keys too, recorded by pressing them.
                  They never reach this page, so the backend reports them
                  once they are let go, while this editor is open. */}
              <p className="flex items-center gap-1.5 text-[12px] text-muted-foreground">
                <Globe className="size-3 shrink-0" aria-hidden />
                Or press modifier keys on their own, like Fn, or Control and Option, then let go.
              </p>
            </div>
          ) : (
            <MouseCapture
              current={trigger.binding?.kind === "mouse" ? trigger.binding : null}
              onCapture={(button) =>
                // Close only if it took. A refused button leaves the editor
                // open with the reason under the row, so another one is a
                // click away.
                setBindingNow({ kind: "mouse", button }).then(
                  onCloseEditor,
                  ignoreHandled,
                )
              }
              onCancel={onCloseEditor}
            />
          )}

          {/* Clearing the field is a way out. A row whose key is taken can be
              left unbound instead of rebound: the backend allows an unbound
              row and switches it off, so whatever was wrong with the binding
              stops being wrong. Both tabs, because a mouse button is a
              binding too. */}
          {trigger.binding && (
            <Button
              variant="link"
              size="sm"
              onClick={() => void setBindingNow(null).catch(ignoreHandled)}
              className="h-auto p-0 text-[12px] text-muted-foreground hover:text-foreground"
            >
              Remove binding
            </Button>
          )}
        </div>
      )}



      {issue && <IssueLine message={issue} className="mt-2" />}
    </div>
  );
}

/* -------------------------------------------------------------------------- */
/* One derived issue                                                          */
/* -------------------------------------------------------------------------- */

/**
 * A field error: a small red icon and the backend's sentence, nothing else.
 * No badge, no dismiss, no timer. It is on screen because the condition holds
 * and it leaves because the condition does.
 */
function IssueLine({
  message,
  className,
}: {
  message: string;
  className?: string;
}) {
  return (
    <p
      role="alert"
      className={cn(
        "flex items-start gap-1.5 text-[12px] leading-snug text-destructive",
        className,
      )}
    >
      <AlertCircle className="mt-[2px] size-3 shrink-0" aria-hidden />
      <span>{message}</span>
    </p>
  );
}

/* -------------------------------------------------------------------------- */
/* The globe key note                                                         */
/* -------------------------------------------------------------------------- */

/**
 * What macOS does with the globe key before Juno gets a say.
 *
 * Shown once per screen, and only when the system still gives the key a job:
 * it is advice about one key, not a standing notice about each row. The link goes
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
        <Button
          variant="link"
          onClick={async () => {
            try {
              await invoke(COMMANDS.TRIGGERS_OPEN_KEYBOARD_SETTINGS);
              setFailed(false);
            } catch (e) {
              console.debug("[Triggers] could not open Keyboard settings:", e);
              setFailed(true);
            }
          }}
          className="inline h-auto p-0 text-[12px] font-normal text-[#007AFF] dark:text-[#0A84FF]"
        >
          System Settings, Keyboard
        </Button>
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
