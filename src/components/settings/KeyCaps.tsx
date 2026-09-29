import { cn } from "@/lib/utils";

/**
 * A shortcut drawn the way macOS draws it: one cap per key, modifiers as
 * their glyphs in Apple's order (⌃ ⌥ ⇧ ⌘), never spelled out. The stored
 * string ("Option+Space") is the backend's; this only decides how it looks.
 */

const MODIFIER_GLYPHS: Record<string, { glyph: string; name: string; order: number }> = {
  ctrl: { glyph: "⌃", name: "Control", order: 0 },
  control: { glyph: "⌃", name: "Control", order: 0 },
  alt: { glyph: "⌥", name: "Option", order: 1 },
  option: { glyph: "⌥", name: "Option", order: 1 },
  shift: { glyph: "⇧", name: "Shift", order: 2 },
  cmd: { glyph: "⌘", name: "Command", order: 3 },
  command: { glyph: "⌘", name: "Command", order: 3 },
  meta: { glyph: "⌘", name: "Command", order: 3 },
  super: { glyph: "⌘", name: "Command", order: 3 },
};

const KEY_GLYPHS: Record<string, { glyph: string; name: string }> = {
  fn: { glyph: "🌐", name: "Globe" },
  globe: { glyph: "🌐", name: "Globe" },
  space: { glyph: "Space", name: "Space" },
  escape: { glyph: "⎋", name: "Escape" },
  esc: { glyph: "⎋", name: "Escape" },
  enter: { glyph: "↩", name: "Return" },
  return: { glyph: "↩", name: "Return" },
  tab: { glyph: "⇥", name: "Tab" },
  backspace: { glyph: "⌫", name: "Delete" },
  delete: { glyph: "⌦", name: "Forward Delete" },
  up: { glyph: "↑", name: "Up Arrow" },
  down: { glyph: "↓", name: "Down Arrow" },
  left: { glyph: "←", name: "Left Arrow" },
  right: { glyph: "→", name: "Right Arrow" },
  arrowup: { glyph: "↑", name: "Up Arrow" },
  arrowdown: { glyph: "↓", name: "Down Arrow" },
  arrowleft: { glyph: "←", name: "Left Arrow" },
  arrowright: { glyph: "→", name: "Right Arrow" },
  home: { glyph: "↖", name: "Home" },
  end: { glyph: "↘", name: "End" },
  pageup: { glyph: "⇞", name: "Page Up" },
  pagedown: { glyph: "⇟", name: "Page Down" },
  capslock: { glyph: "⇪", name: "Caps Lock" },
};

export interface KeyCap {
  glyph: string;
  /** Spoken name, for the accessible label. */
  name: string;
}

/** The caps a shortcut string renders as, modifiers first in Apple's order. */
export function shortcutCaps(shortcut: string): KeyCap[] {
  const parts = shortcut
    .split("+")
    .map((p) => p.trim())
    .filter(Boolean);
  const modifiers: Array<KeyCap & { order: number }> = [];
  const keys: KeyCap[] = [];
  for (const part of parts) {
    const lower = part.toLowerCase();
    const modifier = MODIFIER_GLYPHS[lower];
    if (modifier) {
      if (!modifiers.some((m) => m.order === modifier.order)) modifiers.push(modifier);
      continue;
    }
    const known = KEY_GLYPHS[lower];
    if (known) {
      keys.push(known);
      continue;
    }
    // A letter, digit or function key is its own cap. Single letters show
    // uppercase, the way key caps are printed.
    const glyph = part.length === 1 ? part.toUpperCase() : part;
    keys.push({ glyph, name: glyph });
  }
  modifiers.sort((a, b) => a.order - b.order);
  return [...modifiers.map(({ glyph, name }) => ({ glyph, name })), ...keys];
}

/** "Option Space", for aria labels and logs. */
export function shortcutSpoken(shortcut: string): string {
  return shortcutCaps(shortcut)
    .map((c) => c.name)
    .join(" ");
}

interface KeyCapsProps {
  shortcut: string;
  /** Larger caps for the recorder; the row uses the small size. */
  size?: "sm" | "md";
  /** Drawn pressed, for the recorder while the keys are down. */
  pressed?: boolean;
  className?: string;
}

export function KeyCaps({ shortcut, size = "sm", pressed = false, className }: KeyCapsProps) {
  const caps = shortcutCaps(shortcut);
  if (caps.length === 0) return null;
  return (
    <span
      role="img"
      aria-label={shortcutSpoken(shortcut)}
      className={cn("inline-flex items-center gap-1", className)}
    >
      {caps.map((cap, i) => (
        <kbd
          key={`${cap.glyph}-${i}`}
          aria-hidden="true"
          className={cn(
            "inline-flex items-center justify-center rounded-[5px] border border-border bg-background font-sans text-foreground",
            "shadow-[0_1px_0_rgba(0,0,0,0.08)]",
            size === "sm"
              ? "h-[18px] min-w-[18px] px-1 text-[11px] leading-none"
              : "h-[26px] min-w-[26px] px-1.5 text-[13px] leading-none",
            pressed && "translate-y-px bg-muted shadow-none",
          )}
        >
          {cap.glyph}
        </kbd>
      ))}
    </span>
  );
}
