import { useEffect, useLayoutEffect, useRef, useState, type FormEvent, type ReactNode, type RefObject } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion } from "motion/react";
import { cn } from "@/lib/utils";
import type { ChatMessage } from "@/types/chat";
import { CAPTION_HEIGHT, CAPTION_WIDTH, type Caption, type CaptionTone } from "./orbModel";

/**
 * The words under the orb. One slot: a line of subtitle, a composer, or an
 * approval with Allow and Don't; and, below it, the sheet that unfolds for
 * the whole answer. All of it is dark on any wallpaper, the way subtitles
 * are, so the orb stays the only coloured thing.
 */

/** Hairline plus depth so the pill reads on a black wallpaper too. */
const SHADOW = "0 0 0 0.5px rgba(255,255,255,0.18), 0 1px 2px rgba(0,0,0,0.4), 0 6px 20px rgba(0,0,0,0.35)";
const EASE_IN = { duration: 0.18, ease: "easeOut" as const };
const EASE_OUT = { duration: 0.12, ease: "easeIn" as const };

const TONE_CLASS: Record<CaptionTone, string> = {
  live: "text-white/92",
  dim: "text-white/50",
  provisional: "text-white/60 italic",
  error: "text-[#FF6961]",
  plain: "text-white/80",
};

const KEYFRAMES = `
@keyframes orb-breathe {
  0%, 100% { transform: scale(1); }
  50%      { transform: scale(1.04); }
}
@media (prefers-reduced-motion: reduce) {
  [style*="orb-breathe"] { animation: none !important; }
}
.orb-scroll { scrollbar-width: thin; scrollbar-color: rgba(255,255,255,0.22) transparent; }
.orb-scroll::-webkit-scrollbar { width: 6px; }
.orb-scroll::-webkit-scrollbar-track { background: transparent; }
.orb-scroll::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.22); border-radius: 3px; }
`;

export function useOrbKeyframes() {
  useEffect(() => {
    const id = "orb-keyframes";
    if (document.getElementById(id)) return;
    const style = document.createElement("style");
    style.id = id;
    style.textContent = KEYFRAMES;
    document.head.appendChild(style);
  }, []);
}

/** Fades and rises in after the window has grown; fades out before it shrinks. */
export function CaptionSlot({
  children,
  reducedMotion,
  testId,
}: {
  children: ReactNode;
  reducedMotion: boolean;
  testId: string;
}) {
  return (
    <motion.div
      data-testid={testId}
      initial={reducedMotion ? { opacity: 0 } : { opacity: 0, y: 4 }}
      animate={{ opacity: 1, y: 0, transition: EASE_IN }}
      exit={reducedMotion ? { opacity: 0, transition: EASE_OUT } : { opacity: 0, y: 2, transition: EASE_OUT }}
      className="flex w-full flex-col items-center"
    >
      {children}
    </motion.div>
  );
}

/** One line, like a subtitle. Newest words stay visible: the box clips its
 *  start and fades on the left once there is more than fits. */
export function CaptionLine({ caption }: { caption: Extract<Caption, { kind: "words" }> }) {
  const boxRef = useRef<HTMLDivElement>(null);
  const [overflowing, setOverflowing] = useState(false);
  // Measured again whenever the box changes size: the slot is still fading
  // in when the words first mount, so a one-time measurement would be wrong.
  useLayoutEffect(() => {
    const el = boxRef.current;
    if (!el) return;
    const measure = () => setOverflowing(el.scrollWidth > el.clientWidth + 1);
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, [caption.text]);
  const mask = overflowing ? "linear-gradient(to right, transparent, black 16px)" : undefined;
  return (
    <div
      data-testid="orb-caption"
      data-tone={caption.tone}
      data-overflowing={overflowing ? "true" : "false"}
      className="flex min-w-0 items-center rounded-full bg-black/85 px-3.5"
      style={{ height: CAPTION_HEIGHT, maxWidth: CAPTION_WIDTH, boxShadow: SHADOW }}
    >
      <div
        ref={boxRef}
        className={cn(
          "min-w-0 overflow-hidden whitespace-nowrap text-[13px] leading-none tracking-[-0.01em]",
          TONE_CLASS[caption.tone],
        )}
        style={{ direction: "rtl", maskImage: mask, WebkitMaskImage: mask }}
      >
        <bdi style={{ direction: "ltr", unicodeBidi: "isolate" }}>{caption.text}</bdi>
      </div>
    </div>
  );
}

/** The caption as a text field: Rust is in its input state. */
export function CaptionComposer({
  value,
  disabled,
  inputRef,
  onChange,
  onSubmit,
}: {
  value: string;
  disabled: boolean;
  inputRef: RefObject<HTMLInputElement | null>;
  onChange: (value: string) => void;
  onSubmit: (event: FormEvent) => void;
}) {
  return (
    <form
      onSubmit={onSubmit}
      data-testid="orb-composer"
      className="flex items-center gap-2 rounded-full bg-black/85 pl-4 pr-3"
      style={{ height: CAPTION_HEIGHT, width: CAPTION_WIDTH, boxShadow: SHADOW }}
    >
      <input
        ref={inputRef}
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        aria-label="Ask Juno"
        placeholder="Ask Juno"
        disabled={disabled}
        className={cn(
          "min-w-0 flex-1 bg-transparent text-[13px] tracking-[-0.01em] text-white/92 outline-none",
          "placeholder:text-white/30",
        )}
      />
      <span
        className={cn(
          "shrink-0 select-none text-[11px] tracking-[0.04em] text-white/25 transition-opacity duration-150",
          value.trim() ? "opacity-100" : "opacity-0",
        )}
        aria-hidden="true"
      >
        return
      </span>
    </form>
  );
}

/** The caption becomes the question: what Juno wants to do, Allow, Don't. */
export function CaptionApproval({
  caption,
  msg,
  onDecided,
}: {
  caption: Extract<Caption, { kind: "approval" }>;
  msg: ChatMessage;
  onDecided: (toolId: string, state: "approved" | "denied") => void;
}) {
  const [busy, setBusy] = useState(false);
  const decide = async (state: "approved" | "denied") => {
    if (!msg.tool_id || busy) return;
    setBusy(true);
    try {
      const command = state === "approved" ? "approve_tool_execution" : "deny_tool_execution";
      const ok = await invoke<boolean>(command, { toolId: msg.tool_id });
      if (ok) onDecided(msg.tool_id, state);
    } catch (error) {
      console.error("Orb: approval failed:", error);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div
      data-testid="orb-approval"
      role="group"
      aria-label="Permission"
      className="flex flex-col items-center gap-2 rounded-[18px] bg-black/85 px-4 pb-2.5 pt-2.5"
      style={{ width: CAPTION_WIDTH, boxShadow: SHADOW }}
    >
      <p className="max-w-full truncate text-[13px] leading-none tracking-[-0.01em] text-white/92">
        {caption.text}
      </p>
      <div className="flex items-center gap-2">
        <button
          type="button"
          disabled={busy}
          onClick={() => void decide("approved")}
          className={cn(
            "h-7 rounded-full bg-[#0A84FF] px-3.5 text-[12px] font-medium text-white",
            "transition-colors hover:bg-[#2B93FF] active:bg-[#0071E3] disabled:opacity-50",
          )}
        >
          Allow
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={() => void decide("denied")}
          className={cn(
            "h-7 rounded-full px-3.5 text-[12px] text-white/70",
            "transition-colors hover:bg-white/[0.08] hover:text-white disabled:opacity-50",
          )}
        >
          Don&rsquo;t
        </button>
      </div>
    </div>
  );
}

/** The whole answer, unfolded under the orb. Its height follows the content. */
export function CaptionSheet({
  height,
  measureRef,
  reducedMotion,
  children,
}: {
  height: number;
  measureRef: (el: HTMLDivElement | null) => void;
  reducedMotion: boolean;
  children: ReactNode;
}) {
  return (
    <motion.div
      data-testid="orb-sheet"
      data-no-drag
      initial={false}
      animate={{ height }}
      transition={reducedMotion ? { duration: 0.15 } : { type: "spring", stiffness: 380, damping: 34, mass: 1 }}
      className="dark orb-scroll cursor-auto select-text overflow-y-auto rounded-[16px] bg-black/90 px-4"
      style={{ width: CAPTION_WIDTH, boxShadow: SHADOW }}
    >
      <div ref={measureRef} className="py-3 text-[13px] leading-[1.55] text-white/88">
        {children}
      </div>
    </motion.div>
  );
}
