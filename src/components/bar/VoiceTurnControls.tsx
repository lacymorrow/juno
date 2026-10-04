/**
 * The controls a recording voice turn shows, in every bar appearance.
 *
 * Which controls appear is decided by `voiceTurnControls` (src/lib/voiceTurn.ts);
 * this only draws them. A look passes its own button class so the controls
 * match its chrome, and nothing else: the set, the order, the icons and the
 * words are the same everywhere.
 */
import { useCallback, useEffect, useRef, type MouseEvent as ReactMouseEvent } from "react";
import { ArrowUp, Type, X } from "lucide-react";
import { cn } from "@/lib/utils";
import { UI } from "@/lib/constants.generated";
import {
  VOICE_TURN_LABELS,
  cancelVoiceTurn,
  isRecordingTurn,
  sendVoiceTurn,
  voiceTurnControls,
  type VoiceTurnControl,
  type VoiceTurnState,
} from "@/lib/voiceTurn";

export interface VoiceTurnControlsProps {
  state: VoiceTurnState;
  onSend: () => void;
  onType: () => void;
  onCancel: () => void;
  /** The look's own button class. `send` is the primary action. */
  buttonClassName?: (control: VoiceTurnControl) => string;
  className?: string;
}

const ICONS: Record<VoiceTurnControl, typeof ArrowUp> = {
  send: ArrowUp,
  type: Type,
  cancel: X,
};

const DEFAULT_BUTTON =
  "flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-full text-white/60 transition-colors hover:bg-white/[0.12] hover:text-white";

export function VoiceTurnControls({
  state,
  onSend,
  onType,
  onCancel,
  buttonClassName,
  className,
}: VoiceTurnControlsProps) {
  const controls = voiceTurnControls(state);
  if (controls.length === 0) return null;
  const handlers: Record<VoiceTurnControl, () => void> = {
    send: onSend,
    type: onType,
    cancel: onCancel,
  };
  return (
    <div
      className={cn("flex shrink-0 items-center gap-1", className)}
      role="group"
      aria-label="Voice"
      data-testid="voice-turn-controls"
    >
      {controls.map((control) => {
        const Icon = ICONS[control];
        const label = VOICE_TURN_LABELS[control];
        return (
          <button
            key={control}
            type="button"
            onClick={(e: ReactMouseEvent) => {
              e.stopPropagation();
              handlers[control]();
            }}
            aria-label={label}
            title={label}
            data-voice-control={control}
            className={
              buttonClassName?.(control) ??
              cn(DEFAULT_BUTTON, control === "send" && "bg-white/[0.16] text-white")
            }
          >
            <Icon className="size-3" />
          </button>
        );
      })}
    </div>
  );
}

/**
 * The actions behind those controls, for a look that has no turn logic of its
 * own (Island, Avatar). `openTyping` opens that look's composer; it runs once
 * Rust says the turn is over, because the bar refuses a composer while a voice
 * state is still standing. `stopAll` is the coordinated stop, which is the only
 * thing that reaches a wake-phrase capture.
 */
export function useVoiceTurn(
  state: VoiceTurnState,
  { openTyping, stopAll }: { openTyping: () => void; stopAll: () => void },
) {
  const wantsTypingRef = useRef(false);
  const stateRef = useRef(state);
  stateRef.current = state;

  const send = useCallback(() => void sendVoiceTurn(), []);
  const cancel = useCallback(() => {
    if (stateRef.current.barState === UI.BAR_STATES_ALWAYS_LISTENING) {
      stopAll();
      return;
    }
    void cancelVoiceTurn(stateRef.current);
  }, [stopAll]);
  const typeInstead = useCallback(() => {
    wantsTypingRef.current = true;
    void cancelVoiceTurn(stateRef.current);
  }, []);

  const recording =
    isRecordingTurn(state) || state.barState === UI.BAR_STATES_ALWAYS_LISTENING;
  useEffect(() => {
    if (!wantsTypingRef.current || recording) return;
    wantsTypingRef.current = false;
    openTyping();
  }, [recording, openTyping]);

  return { send, cancel, typeInstead };
}
