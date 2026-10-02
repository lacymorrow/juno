import { useCallback, useEffect, useState } from "react";
import { cn } from "@/lib/utils";
import { Persona } from "@/components/ai-elements/persona";
import { useSpeechLevel } from "@/hooks/useSpeechLevel";
import { HEAD, mouthScale, type Cue, type HeadLook } from "./avatarModel";
import { VOICE_TRANSITION, clampLevel, voiceScale } from "../voiceLevel";

/**
 * The character's head: the Rive sphere (its own listening, thinking and
 * speaking loops), a flat disc under it so the head is there before the
 * network delivers the sphere and stays there if it never does, and the face
 * on top. The Rive file has no eyes, mouth or neck, so the face is SVG and
 * the gestures are transforms on this box. State is told by what the face
 * does, never by an icon.
 *
 * The mouth moves only while Juno's voice is actually audible: it follows the
 * speech level Rust streams from the audio being played, and closes the
 * moment the sound stops, however long the bar stays in a speaking state.
 */

const KEYFRAMES = `
.av-head { transform-origin: 50% 88%; transition: transform 260ms cubic-bezier(.2,.8,.2,1); }
.av-head[data-gesture="lean"]   { transform: rotate(4deg) translateY(var(--av-toward, 2px)) scale(1.04); }
.av-head[data-gesture="attend"] { transform: rotate(2deg); }
.av-head[data-gesture="think"]  { transform: rotate(-4deg) translateY(calc(var(--av-toward, 2px) * -0.5)); }
.av-head[data-gesture="wide"]   { transform: scale(1.03); }
.av-head[data-gesture="nod"]    { animation: av-nod 640ms cubic-bezier(.3,.7,.3,1) 1; }
.av-head[data-gesture="wince"]  { animation: av-wince 420ms cubic-bezier(.2,.8,.2,1) 1 forwards; }

/* The sphere takes the theme's colour (dark glass in light mode, white glass
   in dark mode), so the face is the opposite and the disc matches the sphere. */
.av-face { color: #FFFFFF; }
.av-disc { background: #1C1C1E; box-shadow: 0 0 0 0.5px rgba(255,255,255,0.18), 0 1px 2px rgba(0,0,0,0.4); }
@media (prefers-color-scheme: dark) {
  .av-face { color: #1C1C1E; }
  .av-disc { background: #FFFFFF; box-shadow: 0 0 0 0.5px rgba(0,0,0,0.12), 0 1px 2px rgba(0,0,0,0.12); }
}
.dark .av-face { color: #1C1C1E; }
.dark .av-disc { background: #FFFFFF; box-shadow: 0 0 0 0.5px rgba(0,0,0,0.12), 0 1px 2px rgba(0,0,0,0.12); }

.av-eyes { transform-box: fill-box; transform-origin: center; transition: transform 220ms cubic-bezier(.2,.8,.2,1); }
.av-eyes[data-blink="true"] { animation: av-blink 4.4s ease-in-out infinite; }
.av-head[data-gesture="lean"] .av-eyes,
.av-head[data-gesture="attend"] .av-eyes { transform: translate(2px, var(--av-look, 1.5px)); }
.av-head[data-gesture="think"] .av-eyes { transform: translate(-2px, calc(var(--av-look, 1.5px) * -1.6)); }
.av-head[data-gesture="wide"] .av-eyes { transform: scale(1.22); animation: none; }
.av-head[data-gesture="wince"] .av-eyes { transform: scaleY(0.38); animation: none; }
.av-head[data-gesture="nod"] .av-eyes { animation: none; }

.av-mouth { transform-box: fill-box; transform-origin: center; transform: scaleY(0.3); transition: transform 180ms ease-out; }
.av-mouth[data-talking="true"] { transition: transform 70ms linear; }
.av-head[data-gesture="wince"] .av-mouth[data-talking="false"] { transform: scaleY(0.3) scaleX(0.7) translateY(1px); }

@keyframes av-blink {
  0%, 90%, 100% { transform: scaleY(1); }
  94% { transform: scaleY(0.08); }
}
@keyframes av-nod {
  0%   { transform: translateY(0) scaleY(1); }
  35%  { transform: translateY(var(--av-toward, 2px)) scaleY(0.96); }
  60%  { transform: translateY(0) scaleY(1); }
  80%  { transform: translateY(calc(var(--av-toward, 2px) * 0.5)) scaleY(0.98); }
  100% { transform: translateY(0) scaleY(1); }
}
@keyframes av-wince {
  0%   { transform: rotate(0deg) scale(1); }
  40%  { transform: rotate(-6deg) scale(0.94) translateX(-2px); }
  100% { transform: rotate(-3deg) scale(0.97); }
}
@keyframes av-cue-breathe {
  0%, 100% { opacity: 0.6; transform: scale(1); }
  50%      { opacity: 1; transform: scale(1.3); }
}
@keyframes av-cue-slow {
  0%, 100% { opacity: 0.35; transform: scale(1); }
  50%      { opacity: 0.6; transform: scale(1.1); }
}
/* Reduce Motion is a system setting, not a per-face choice. */
@media (prefers-reduced-motion: reduce) {
  .av-head, .av-eyes, .av-mouth, .av-cue { animation: none !important; transition: none !important; }
}
`;

function useHeadKeyframes() {
  useEffect(() => {
    const id = "avatar-head-keyframes";
    if (document.getElementById(id)) return;
    const style = document.createElement("style");
    style.id = id;
    style.textContent = KEYFRAMES;
    document.head.appendChild(style);
  }, []);
}

const CUE_ANIMATION: Record<Cue["motion"], string | undefined> = {
  breathe: "av-cue-breathe 1.4s ease-in-out infinite",
  slow: "av-cue-slow 4s ease-in-out infinite",
  still: undefined,
};

/**
 * The face: two eyes that blink and look, and a mouth that opens with Juno's
 * voice. It subscribes to the speech level itself, so only this SVG redraws
 * at the level's rate, not the sphere or the bar around it.
 */
function Face({ blink, reducedMotion }: { blink: boolean; reducedMotion: boolean }) {
  const voice = useSpeechLevel();
  const open = mouthScale({ ...voice, reducedMotion });
  return (
    <svg
      className="av-face pointer-events-none absolute inset-0"
      viewBox="0 0 72 72"
      width={HEAD}
      height={HEAD}
      aria-hidden="true"
      data-testid="avatar-face"
    >
      <g className="av-eyes" data-blink={blink ? "true" : "false"} fill="currentColor">
        <rect x="25.5" y="28" width="5" height="11" rx="2.5" />
        <rect x="41.5" y="28" width="5" height="11" rx="2.5" />
      </g>
      <ellipse
        className="av-mouth"
        data-testid="avatar-mouth"
        data-talking={voice.speaking ? "true" : "false"}
        data-open={open.toFixed(2)}
        style={voice.speaking ? { transform: `scaleY(${open})` } : undefined}
        cx="36"
        cy="47.5"
        rx="5"
        ry="3"
        fill="currentColor"
      />
    </svg>
  );
}

interface AvatarHeadProps {
  look: HeadLook;
  /** Bubbles rise above the head: the lean and the look flip to face them. */
  facingUp: boolean;
  reducedMotion: boolean;
  /** The mic level while Juno listens (0..1). The cue by its ear swells with
   *  your voice and a soft ring spreads from it; 0 leaves the cue as it is. */
  level?: number;
  onClick?: () => void;
  className?: string;
}

export function AvatarHead({ look, facingUp, reducedMotion, level = 0, onClick, className }: AvatarHeadProps) {
  useHeadKeyframes();
  const swell = voiceScale(level, 1.1);
  const heard = clampLevel(level);
  // The sphere arrives over the network. Until it has, and if it never does,
  // the disc is the head.
  const [sphere, setSphere] = useState<"loading" | "ready" | "failed">("loading");
  const onLoad = useCallback(() => setSphere("ready"), []);
  const onLoadError = useCallback(() => setSphere("failed"), []);

  const blink = !reducedMotion && look.gesture !== "wince" && look.gesture !== "nod" && look.gesture !== "wide";

  return (
    <div
      className={cn("av-head relative select-none", className)}
      style={{
        width: HEAD,
        height: HEAD,
        // Toward you is down when the bubbles hang below the head, up when
        // they rise above it.
        ["--av-toward" as string]: facingUp ? "-2px" : "2px",
        ["--av-look" as string]: facingUp ? "-1.5px" : "1.5px",
      }}
      data-testid="avatar-head"
      data-gesture={look.gesture}
      data-rive={look.rive}
      data-sphere={sphere}
      onClick={onClick}
      role={onClick ? "button" : undefined}
      aria-label={onClick ? "Ask Juno" : undefined}
      tabIndex={onClick ? 0 : undefined}
      onKeyDown={
        onClick
          ? (e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onClick();
              }
            }
          : undefined
      }
    >
      <div
        className="av-disc absolute rounded-full transition-opacity duration-300"
        style={{ inset: HEAD * 0.07, opacity: sphere === "ready" ? 0 : 1 }}
        aria-hidden="true"
      />
      {sphere !== "failed" && (
        <Persona
          state={look.rive}
          variant="obsidian"
          onLoad={onLoad}
          onLoadError={onLoadError}
          className={cn(
            "absolute inset-0 !size-[84px] transition-opacity duration-300",
            sphere === "ready" ? "opacity-100" : "opacity-0",
          )}
        />
      )}
      <Face blink={blink} reducedMotion={reducedMotion} />
      {look.cue && (
        // The outer span carries the voice swell; the inner one keeps the
        // cue's own breathing, so the two transforms never fight.
        <span
          className="absolute flex items-center justify-center"
          style={{
            width: 7,
            height: 7,
            right: HEAD * 0.03,
            top: HEAD / 2 - 3.5,
            transform: `scale(${swell})`,
            transition: VOICE_TRANSITION,
          }}
          data-testid="avatar-cue-swell"
          data-swell={swell.toFixed(2)}
          aria-hidden="true"
        >
          {heard > 0 && (
            <span
              className="absolute block rounded-full"
              style={{
                inset: -3,
                backgroundColor: look.cue.color,
                opacity: 0.25 * heard,
              }}
            />
          )}
          <span
            className="av-cue relative block rounded-full"
            data-testid="avatar-cue"
            data-motion={look.cue.motion}
            style={{
              width: 7,
              height: 7,
              backgroundColor: look.cue.color,
              opacity: look.cue.opacity,
              animation: reducedMotion ? undefined : CUE_ANIMATION[look.cue.motion],
            }}
          />
        </span>
      )}
    </div>
  );
}
