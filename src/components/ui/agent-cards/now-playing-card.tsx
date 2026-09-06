/**
 * NowPlayingCard — a LIVE media card bound to the real player.
 *
 * The rule this component exists to enforce: an agent-rendered control that
 * implies live state (a play/pause toggle, a progress bar, a "Now Playing"
 * label) must be fed by live state. A QueryButton that fires a new agent run
 * cannot know whether playback changed and takes seconds to round-trip, so
 * it is never an acceptable play/pause button.
 *
 * This card polls `media_get_state` while mounted and sends transport
 * actions through `media_control`, which talks to Spotify / Music over
 * AppleScript and returns the state after the action. The UI updates
 * optimistically and is reconciled by the next poll.
 */

import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { COMMANDS } from "@/lib/constants.generated";
import { useCallback, useEffect, useRef, useState } from "react";
import { Loader2, Music2, Pause, Play, SkipBack, SkipForward } from "lucide-react";

export type MediaApp = "Spotify" | "Music";
export type MediaAction = "play" | "pause" | "playpause" | "next" | "previous";

export interface MediaState {
  app: string;
  running: boolean;
  state: "playing" | "paused" | "stopped" | "not_running" | string;
  track: string | null;
  artist: string | null;
  album: string | null;
  position_secs: number | null;
  duration_secs: number | null;
  artwork_url: string | null;
}

export interface NowPlayingCardProps {
  /** Player to bind to. Defaults to Spotify. */
  app?: MediaApp;
  /** Poll interval in milliseconds while the card is visible. */
  pollMs?: number;
  /** Optional heading; defaults to "Now Playing". */
  title?: string;
  className?: string;
}

const DEFAULT_POLL_MS = 1000;

function formatTime(secs: number | null): string {
  if (secs == null || !Number.isFinite(secs) || secs < 0) return "0:00";
  const whole = Math.floor(secs);
  const m = Math.floor(whole / 60);
  const s = whole % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

async function getMediaState(app: MediaApp): Promise<MediaState> {
  return invoke<MediaState>(COMMANDS.MEDIA_GET_STATE, { app });
}

async function controlMedia(app: MediaApp, action: MediaAction): Promise<MediaState> {
  return invoke<MediaState>(COMMANDS.MEDIA_CONTROL, { app, action });
}

export function NowPlayingCard({
  app = "Spotify",
  pollMs = DEFAULT_POLL_MS,
  title = "Now Playing",
  className,
}: NowPlayingCardProps) {
  const [state, setState] = useState<MediaState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<MediaAction | null>(null);
  // Artwork URL that failed to load; never show a blank square for it.
  const [brokenArtwork, setBrokenArtwork] = useState<string | null>(null);
  const mounted = useRef(true);

  const refresh = useCallback(async () => {
    try {
      const next = await getMediaState(app);
      if (!mounted.current) return;
      setState(next);
      setError(null);
    } catch (err) {
      if (!mounted.current) return;
      setError(String(err));
    }
  }, [app]);

  // Poll while mounted and visible. The player is the source of truth; the
  // card never invents state it has not read back.
  useEffect(() => {
    mounted.current = true;
    void refresh();

    let timer: ReturnType<typeof setInterval> | null = null;
    const start = () => {
      if (timer == null) timer = setInterval(() => void refresh(), pollMs);
    };
    const stop = () => {
      if (timer != null) {
        clearInterval(timer);
        timer = null;
      }
    };
    const onVisibility = () => {
      if (document.visibilityState === "hidden") stop();
      else {
        void refresh();
        start();
      }
    };

    start();
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      mounted.current = false;
      stop();
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [refresh, pollMs]);

  const act = useCallback(
    async (action: MediaAction) => {
      if (pending) return;
      setPending(action);
      // Optimistic flip for play/pause so the button answers instantly; the
      // real state from the backend replaces it as soon as it returns.
      if (action === "play" || action === "pause") {
        setState((prev) =>
          prev ? { ...prev, state: action === "play" ? "playing" : "paused" } : prev,
        );
      }
      try {
        const next = await controlMedia(app, action);
        if (!mounted.current) return;
        setState(next);
        setError(null);
      } catch (err) {
        if (!mounted.current) return;
        setError(String(err));
        void refresh();
      } finally {
        if (mounted.current) setPending(null);
      }
    },
    [app, pending, refresh],
  );

  const openApp = useCallback(async () => {
    try {
      await invoke("open_application", { appName: app });
      setTimeout(() => void refresh(), 1500);
    } catch (err) {
      setError(String(err));
    }
  }, [app, refresh]);

  const isPlaying = state?.state === "playing";
  const hasTrack = !!state?.track;
  const position = state?.position_secs ?? null;
  const duration = state?.duration_secs ?? null;
  const progress =
    position != null && duration != null && duration > 0
      ? Math.min(100, Math.max(0, (position / duration) * 100))
      : null;

  const controlButton = (
    action: MediaAction,
    Icon: React.ComponentType<{ className?: string }>,
    label: string,
    primary = false,
  ) => (
    <button
      type="button"
      onClick={() => void act(action)}
      disabled={!state?.running || pending != null}
      aria-label={label}
      title={label}
      className={cn(
        "inline-flex items-center justify-center rounded-full transition-colors",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        "disabled:pointer-events-none disabled:opacity-40",
        primary
          ? "h-9 w-9 bg-primary text-primary-foreground hover:bg-primary/90"
          : "h-8 w-8 hover:bg-accent hover:text-accent-foreground",
      )}
    >
      {pending === action ? (
        <Loader2 className="h-4 w-4 animate-spin" />
      ) : (
        <Icon className="h-4 w-4" />
      )}
    </button>
  );

  return (
    <div
      className={cn(
        "rounded-xl border bg-card p-4 juno-animate-in shadow-sm",
        className,
      )}
      data-testid="now-playing-card"
      data-media-state={state?.state ?? "loading"}
    >
      <div className="flex items-center justify-between mb-3">
        <div className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
          {title} · {app}
        </div>
        {state && (
          <span
            className={cn(
              "text-[10px] px-2 py-0.5 rounded-full font-medium",
              isPlaying
                ? "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300"
                : state.running
                  ? "bg-yellow-100 text-yellow-700 dark:bg-yellow-900 dark:text-yellow-300"
                  : "bg-muted text-muted-foreground",
            )}
          >
            {!state.running
              ? "Not running"
              : isPlaying
                ? "Playing"
                : state.state === "paused"
                  ? "Paused"
                  : "Stopped"}
          </span>
        )}
      </div>

      {state == null && !error ? (
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="h-4 w-4 animate-spin" /> Reading {app}…
        </div>
      ) : !state?.running ? (
        <div className="flex items-center justify-between gap-3">
          <div className="text-sm text-muted-foreground">{app} isn&apos;t running.</div>
          <button
            type="button"
            onClick={() => void openApp()}
            className="h-7 px-2.5 text-xs rounded-md border border-input bg-background hover:bg-accent hover:text-accent-foreground"
          >
            Open {app}
          </button>
        </div>
      ) : (
        <div className="flex items-center gap-3">
          <div className="h-12 w-12 shrink-0 rounded-md bg-muted overflow-hidden flex items-center justify-center">
            {state.artwork_url && state.artwork_url !== brokenArtwork ? (
              <img
                src={state.artwork_url}
                alt=""
                className="h-full w-full object-cover"
                onError={() => setBrokenArtwork(state.artwork_url)}
              />
            ) : (
              <Music2 className="h-5 w-5 text-muted-foreground" />
            )}
          </div>
          <div className="min-w-0 flex-1">
            <div className="font-medium truncate">
              {hasTrack ? state.track : "Nothing loaded"}
            </div>
            {state.artist && (
              <div className="text-xs text-muted-foreground truncate">
                {state.artist}
                {state.album ? ` — ${state.album}` : ""}
              </div>
            )}
            {progress != null && (
              <div className="mt-1.5">
                <div className="h-1 w-full rounded-full bg-muted overflow-hidden">
                  <div
                    className="h-full rounded-full bg-primary transition-[width] duration-700 ease-linear"
                    style={{ width: `${progress}%` }}
                  />
                </div>
                <div className="flex justify-between text-[10px] text-muted-foreground mt-0.5 tabular-nums">
                  <span>{formatTime(position)}</span>
                  <span>{formatTime(duration)}</span>
                </div>
              </div>
            )}
          </div>
          <div className="flex items-center gap-1 shrink-0">
            {controlButton("previous", SkipBack, "Previous track")}
            {isPlaying
              ? controlButton("pause", Pause, "Pause", true)
              : controlButton("play", Play, "Play", true)}
            {controlButton("next", SkipForward, "Next track")}
          </div>
        </div>
      )}

      {error && (
        <div className="mt-2 text-[10px] text-destructive truncate" role="alert">
          {error}
        </div>
      )}
    </div>
  );
}
