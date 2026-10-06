import { useRef } from "react";
import { UI } from "@/lib/constants.generated";
import { cn } from "@/lib/utils";

/**
 * One row of color swatches for the glow around Juno's cursor, shaped like
 * the accent color row in macOS System Settings. The choices and their colors
 * come from Rust (`constants::ui::agent_cursor_colors`); this only draws them.
 */

const NAMES: Record<string, string> = {
  [UI.AGENT_CURSOR_COLORS_BLUE]: "Blue",
  [UI.AGENT_CURSOR_COLORS_PINK]: "Pink",
  [UI.AGENT_CURSOR_COLORS_GREEN]: "Green",
  [UI.AGENT_CURSOR_COLORS_ORANGE]: "Orange",
  [UI.AGENT_CURSOR_COLORS_PURPLE]: "Purple",
};

const HEX: Record<string, string> = {
  [UI.AGENT_CURSOR_COLORS_BLUE]: UI.AGENT_CURSOR_COLORS_BLUE_HEX,
  [UI.AGENT_CURSOR_COLORS_PINK]: UI.AGENT_CURSOR_COLORS_PINK_HEX,
  [UI.AGENT_CURSOR_COLORS_GREEN]: UI.AGENT_CURSOR_COLORS_GREEN_HEX,
  [UI.AGENT_CURSOR_COLORS_ORANGE]: UI.AGENT_CURSOR_COLORS_ORANGE_HEX,
  [UI.AGENT_CURSOR_COLORS_PURPLE]: UI.AGENT_CURSOR_COLORS_PURPLE_HEX,
};

export const CURSOR_COLOR_IDS: readonly string[] = UI.AGENT_CURSOR_COLORS_ALL;

export function cursorColorHex(id: string): string {
  return HEX[id] ?? UI.AGENT_CURSOR_COLORS_BLUE_HEX;
}

export function CursorColorPicker({
  value,
  onChange,
  disabled = false,
  labelledBy,
}: {
  value: string;
  onChange: (id: string) => void;
  disabled?: boolean;
  labelledBy?: string;
}) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const selected = CURSOR_COLOR_IDS.includes(value) ? value : UI.AGENT_CURSOR_COLORS_DEFAULT;

  const move = (from: number, step: number) => {
    const next = (from + step + CURSOR_COLOR_IDS.length) % CURSOR_COLOR_IDS.length;
    refs.current[next]?.focus();
    onChange(CURSOR_COLOR_IDS[next]);
  };

  return (
    <div role="radiogroup" aria-labelledby={labelledBy} className="flex items-center gap-2">
      {CURSOR_COLOR_IDS.map((id, i) => {
        const checked = id === selected;
        return (
          <button
            key={id}
            ref={(el) => {
              refs.current[i] = el;
            }}
            type="button"
            role="radio"
            aria-checked={checked}
            aria-label={NAMES[id] ?? id}
            title={NAMES[id] ?? id}
            tabIndex={checked ? 0 : -1}
            disabled={disabled}
            onClick={() => onChange(id)}
            onKeyDown={(e) => {
              if (e.key === "ArrowRight" || e.key === "ArrowDown") {
                e.preventDefault();
                move(i, 1);
              } else if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
                e.preventDefault();
                move(i, -1);
              }
            }}
            className={cn(
              // 24px hit area around a 16px swatch.
              "relative grid size-6 place-items-center rounded-full outline-none",
              "focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1",
              "disabled:opacity-50",
            )}
          >
            <span
              aria-hidden="true"
              className={cn(
                "block size-4 rounded-full shadow-[inset_0_0_0_0.5px_rgba(0,0,0,0.2)]",
                checked && "ring-2 ring-offset-2 ring-offset-card",
              )}
              style={{
                backgroundColor: cursorColorHex(id),
                // The selection ring wears the swatch's own color.
                ["--tw-ring-color" as string]: cursorColorHex(id),
              }}
            >
              {checked && (
                <span className="absolute left-1/2 top-1/2 size-1.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-white" />
              )}
            </span>
          </button>
        );
      })}
    </div>
  );
}
