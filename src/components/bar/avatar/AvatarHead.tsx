import { useCallback, useEffect, useState } from "react";
import { cn } from "@/lib/utils";
import { Persona } from "@/components/ai-elements/persona";
import { HEAD, type Cue, type Gesture, type HeadLook } from "./avatarModel";

/**
 * The character's head: the Rive sphere (its own listening, thinking and
 * speaking loops), a flat disc under it so the head is there before the
 * network delivers the sphere and stays there if it never does, and the face
 * on top. The Rive file has no eyes, mouth or neck, so the face is SVG and
 * the gestures are transforms on this box. State is told by what the face
 * does, never by an icon.
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
.av-head[data-gesture="talk"] .av-mouth { animation: av-talk 760ms ease-in-out infinite; }
.av-head[data-gesture="wince"] .av-mouth { transform: scaleY(0.3) scaleX(0.7) translateY(1px); }

@keyframes av-blink {
  0%, 90%, 100% { transform: scaleY(1); }
  94% { transform: scaleY(0.08); }
}
@keyframes av-talk {
  0%   { transform: scaleY(0.3); }
  20%  { transform: scaleY(1.0); }
  40%  { transform: scaleY(0.45); }
  60%  { transform: scaleY(1.2); }
  80%  { transform: scaleY(0.6); }
  100% { transform: scaleY(0.3); }
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

/** The face: two eyes that blink and look, a mouth that moves while talking. */
function Face({ gesture, blink }: { gesture: Gesture; blink: boolean }) {
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
        data-talking={gesture === "talk" ? "true" : "false"}
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
  onClick?: () => void;
  className?: string;
}

export function AvatarHead({ look, facingUp, reducedMotion, onClick, className }: AvatarHeadProps) {
  useHeadKeyframes();
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
      <Face gesture={look.gesture} blink={blink} />
      {look.cue && (
        <span
          className="av-cue absolute block rounded-full"
          data-testid="avatar-cue"
          data-motion={look.cue.motion}
          aria-hidden="true"
          style={{
            width: 7,
            height: 7,
            right: HEAD * 0.03,
            top: HEAD / 2 - 3.5,
            backgroundColor: look.cue.color,
            opacity: look.cue.opacity,
            animation: reducedMotion ? undefined : CUE_ANIMATION[look.cue.motion],
          }}
        />
      )}
    </div>
  );
}
