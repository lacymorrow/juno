import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Check } from "lucide-react";
import { cn } from "@/lib/utils";
import { KeyCaps } from "./KeyCaps";
import { COMMANDS } from "@/lib/constants.generated";

/**
 * Records a keyboard shortcut by pressing it. Recording starts the moment
 * the recorder appears and the chord saves when the keys are let go, the way
 * Wispr Flow and Raycast do it. There is nothing to type and nothing to
 * click: Backspace clears the chord, Escape closes the recorder, Return saves
 * a chord that is already showing.
 *
 * Validation and conflicts are the backend's: it names the row that already
 * holds the combo, and that sentence is what shows here.
 */

interface ShortcutRecorderProps {
  /** The binding this row holds now, for the caps shown before recording. */
  value: string;
  /** Row identity for the backend's own-row check. */
  shortcutName: string;
  /** Persist. Throws with the backend's sentence on a conflict. */
  onSave: (shortcut: string) => Promise<void>;
  /** Close without saving. */
  onCancel: () => void;
  /** The factory binding for this row, when it has one. */
  defaultShortcut?: string | null;
}

const SAVED_HOLD_MS = 1500;

const MODIFIER_CODES = new Set([
  "ShiftLeft",
  "ShiftRight",
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
  "CapsLock",
  "Fn",
  "FnLock",
]);

/** The backend's spelling of the key a DOM event names, or "" for none. */
function keyFromEvent(e: React.KeyboardEvent): string {
  switch (e.code) {
    case "Space":
      return "Space";
    case "Tab":
      return "Tab";
    case "Delete":
      return "Delete";
    case "Home":
      return "Home";
    case "End":
      return "End";
    case "PageUp":
      return "PageUp";
    case "PageDown":
      return "PageDown";
    case "Insert":
      return "Insert";
    case "ArrowUp":
      return "Up";
    case "ArrowDown":
      return "Down";
    case "ArrowLeft":
      return "Left";
    case "ArrowRight":
      return "Right";
    default:
      if (/^F\d{1,2}$/.test(e.code)) return e.code;
      if (e.code.startsWith("Digit")) return e.code.replace("Digit", "");
      if (e.code.startsWith("Key")) return e.code.replace("Key", "");
      if (e.code.startsWith("Numpad")) return e.code;
      if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) return e.key;
      return "";
  }
}

function modifiersFromEvent(e: React.KeyboardEvent): string[] {
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Option");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Cmd");
  return mods;
}

type Phase = "recording" | "checking" | "saved";

export function ShortcutRecorder({
  value,
  shortcutName,
  onSave,
  onCancel,
  defaultShortcut = null,
}: ShortcutRecorderProps) {
  const areaRef = useRef<HTMLDivElement>(null);
  const [phase, setPhase] = useState<Phase>("recording");
  /** The chord currently held, or the last one seen before release. */
  const [chord, setChordState] = useState<string>("");
  // Keydown and keyup can land in one tick (a fast tap, a test); the ref is
  // what keyup reads so it never sees the render before the keydown.
  const chordRef = useRef("");
  const setChord = (next: string) => {
    chordRef.current = next;
    setChordState(next);
  };
  const [held, setHeld] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const savedTimer = useRef<number | null>(null);

  // Recording starts on arrival: focus the area so the first press lands here.
  useEffect(() => {
    areaRef.current?.focus();
    return () => {
      if (savedTimer.current) window.clearTimeout(savedTimer.current);
    };
  }, []);

  const commit = useCallback(
    async (shortcut: string) => {
      setPhase("checking");
      setProblem(null);
      try {
        // The backend's own validation runs on save too; asking first keeps
        // the message identical to the one a save would fail with.
        await invoke<string>(COMMANDS.SHORTCUTS_VALIDATE_KEYBOARD_SHORTCUT, {
          shortcutValue: shortcut,
          shortcutName,
        });
        await onSave(shortcut);
        setPhase("saved");
        savedTimer.current = window.setTimeout(() => onCancel(), SAVED_HOLD_MS);
      } catch (error) {
        const reason =
          typeof error === "string"
            ? error
            : error instanceof Error
              ? error.message
              : "That shortcut cannot be used.";
        setProblem(reason);
        setChord("");
        setPhase("recording");
        areaRef.current?.focus();
      }
    },
    [onCancel, onSave, shortcutName],
  );

  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (phase !== "recording") return;
    e.preventDefault();
    e.stopPropagation();

    if (e.code === "Escape" && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey) {
      onCancel();
      return;
    }
    if (e.code === "Backspace" && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey) {
      setChord("");
      setProblem(null);
      return;
    }
    if ((e.code === "Enter" || e.code === "NumpadEnter") && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey) {
      if (chordRef.current) void commit(chordRef.current);
      return;
    }

    const mods = modifiersFromEvent(e);
    const key = MODIFIER_CODES.has(e.code) ? "" : keyFromEvent(e);
    setHeld(true);
    setProblem(null);
    setChord([...mods, key].filter(Boolean).join("+"));
  };

  const onKeyUp = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (phase !== "recording") return;
    e.preventDefault();
    // Letting go of the main key is the moment the chord is meant. A chord
    // that is only modifiers is not a shortcut; keep waiting for the key.
    const released = MODIFIER_CODES.has(e.code) ? "" : keyFromEvent(e);
    const current = chordRef.current;
    if (!released || !current) {
      if (!e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) setHeld(false);
      return;
    }
    setHeld(false);
    void commit(current);
  };

  const showing = chord || value;

  return (
    <div className="space-y-2">
      <div
        ref={areaRef}
        role="textbox"
        aria-label="Shortcut"
        aria-readonly="true"
        aria-describedby={`recorder-${shortcutName}-hint`}
        tabIndex={0}
        onKeyDown={onKeyDown}
        onKeyUp={onKeyUp}
        onBlur={() => setHeld(false)}
        className={cn(
          "flex min-h-[44px] items-center justify-between gap-3 rounded-[8px] border bg-background px-3 py-2 outline-none",
          "focus-visible:border-[#007AFF] focus-visible:ring-2 focus-visible:ring-[#007AFF]/25",
          problem ? "border-destructive/60" : "border-border",
        )}
      >
        {showing ? (
          <KeyCaps shortcut={showing} size="md" pressed={held} />
        ) : (
          <span className="text-[13px] text-muted-foreground">Press the keys you want</span>
        )}
        {phase === "saved" && (
          <span className="flex items-center gap-1 text-[12px] text-muted-foreground" aria-live="polite">
            <Check className="size-3.5" strokeWidth={2} />
            Shortcut saved
          </span>
        )}
      </div>

      <p id={`recorder-${shortcutName}-hint`} className={cn("text-[12px] leading-snug", problem ? "text-destructive" : "text-muted-foreground")} aria-live="polite">
        {problem ?? (phase === "checking" ? "Checking…" : "They save when you let go. Backspace clears, Escape leaves it as it was.")}
      </p>

      <div className="flex items-center gap-3 text-[12px]">
        <button type="button" onClick={onCancel} className="text-muted-foreground hover:text-foreground">
          Cancel
        </button>
        {defaultShortcut && defaultShortcut !== value && (
          <button
            type="button"
            onClick={() => void commit(defaultShortcut)}
            className="text-muted-foreground hover:text-foreground"
            title="No undo"
          >
            Reset to default
          </button>
        )}
      </div>
    </div>
  );
}
