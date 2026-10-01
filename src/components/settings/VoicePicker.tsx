import { Check, Volume2 } from "lucide-react";
import { cn } from "@/lib/utils";
import type { JunoVoiceOption } from "@/hooks/useSettings";

interface VoicePickerProps {
  options: JunoVoiceOption[];
  /** Pick a voice. Picking is what plays it. */
  onChange: (id: string) => void;
  /** Hear the voice already chosen again. */
  onReplay: () => void;
  /** The row that is speaking right now, if any. */
  speakingId: string | null;
}

/**
 * Pick Juno's voice by hearing it.
 *
 * The bar appearance picker works because the list is short, each entry has a
 * name and one honest line, and you can see what you are choosing. A voice
 * cannot be seen, so this does the same thing with sound: tapping a row saves
 * it and speaks a sample in that voice, and tapping the chosen row again plays
 * it once more. There is no confirm and no toast. Hearing it is the feedback.
 *
 * Rust owns the list, the order, which row is in force and the speaking. This
 * draws what it was given.
 */
export function VoicePicker({ options, onChange, onReplay, speakingId }: VoicePickerProps) {
  if (options.length === 0) {
    return (
      <p className="px-0.5 text-[12px] leading-snug text-muted-foreground" aria-live="polite">
        Juno could not read this Mac's voices.
      </p>
    );
  }

  return (
    // No card of its own: it is already inside one, and the hairlines are the
    // same ones the rows above it use.
    <div role="radiogroup" aria-label="Juno's voice" className="-mx-4 -my-2.5">
      <div className="divide-y divide-border">
        {options.map((option) => {
          const speaking = speakingId === option.id;
          return (
            <button
              key={option.id}
              type="button"
              role="radio"
              aria-checked={option.selected}
              onClick={() => (option.selected ? onReplay() : onChange(option.id))}
              className={cn(
                "flex w-full items-center gap-3 px-4 py-2 text-left",
                "hover:bg-accent/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/60",
              )}
            >
              <span
                aria-hidden="true"
                className={cn(
                  "flex size-4 shrink-0 items-center justify-center",
                  option.selected ? "text-[#007AFF]" : "text-transparent",
                )}
              >
                <Check className="size-4" strokeWidth={2.5} />
              </span>
              <span className="min-w-0 flex-1 space-y-0.5">
                <span className="block text-[13px] font-medium leading-tight">{option.name}</span>
                <span className="block text-[12px] leading-snug text-muted-foreground">
                  {option.descriptor}
                </span>
              </span>
              {option.speaks && (
                <span
                  className={cn(
                    "shrink-0 text-[11px]",
                    speaking ? "text-[#007AFF]" : "text-muted-foreground/60",
                  )}
                >
                  {speaking ? (
                    <span className="flex items-center gap-1">
                      <Volume2 className="size-3.5" strokeWidth={2} aria-hidden="true" />
                      Speaking
                    </span>
                  ) : (
                    <Volume2 className="size-3.5" strokeWidth={2} aria-label="Plays when you pick it" />
                  )}
                </span>
              )}
            </button>
          );
        })}
      </div>
    </div>
  );
}
