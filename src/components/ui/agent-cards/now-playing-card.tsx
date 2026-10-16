/**
 * NowPlayingCard: a LIVE media card bound to the real player.
 *
 * The rule this component exists to enforce: an agent-rendered control that
 * implies live state (a play/pause toggle, a progress bar, a "Now Playing"
 * label) must be fed by live state. A QueryButton that fires a new agent run
 * cannot know whether playback changed and takes seconds to round-trip, so
 * it is never an acceptable play/pause button.
 *
 * This card polls `media_get_state` while mounted and sends transport
 * actions through `media_control`, which talks to Spotify / Apple Music over
 * AppleScript and returns the state after the action. The UI updates
 * optimistically and is reconciled by the next poll. Artwork is whatever URL
 * the backend hands back: Spotify's CDN, or an `asset://` URL for artwork the
 * backend exported from Music.app.
 */

import { invoke } from "@tauri-apps/api/core";
import { Music2, Pause, Play, SkipBack, SkipForward } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardAction, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { Skeleton } from "@/components/ui/skeleton";
import { COMMANDS } from "@/lib/constants.generated";
import { cn } from "@/lib/utils";

/** AppleScript application names, `Music` is Apple Music. */
export type MediaApp = "Spotify" | "Music";

/** What the user sees; the backend addresses the app by its script name. */
const APP_LABELS: Record<MediaApp, string> = {
  Spotify: "Spotify",
  Music: "Apple Music",
};

export function mediaAppLabel(app: MediaApp): string {
  return APP_LABELS[app] ?? app;
}
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
  className,
}: NowPlayingCardProps) {
  const [state, setState] = useState<MediaState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<MediaAction | null>(null);
  // Artwork URL that failed to load; never show a blank square for it.
  const [brokenArtwork, setBrokenArtwork] = useState<string | null>(null);
  const mounted = useRef(true);
  const appLabel = mediaAppLabel(app);

  // `keepError` is set by the post-failure refresh in `act`: the state read
  // usually succeeds, and clearing the error there would wipe the message
  // before the user ever sees it. The next poll clears it as usual.
  const refresh = useCallback(async (opts?: { keepError?: boolean }) => {
    try {
      const next = await getMediaState(app);
      if (!mounted.current) return;
      setState(next);
      if (!opts?.keepError) setError(null);
    } catch (err) {
      // The person sees "Couldn't reach Spotify"; the raw error is for the log.
      console.error("NowPlayingCard: could not read the player:", err);
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
        console.error("NowPlayingCard: transport action failed:", err);
        if (!mounted.current) return;
        setError(String(err));
        void refresh({ keepError: true });
      } finally {
        if (mounted.current) setPending(null);
      }
    },
    [app, pending, refresh],
  );

  const openApp = useCallback(async () => {
    try {
      await invoke(COMMANDS.DESKTOP_OPEN_APPLICATION, { appName: app });
      setTimeout(() => void refresh(), 1500);
    } catch (err) {
      console.error("NowPlayingCard: could not open the player:", err);
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

  // The glyph is the whole control: no disc, no fill, the label colour.
  const controlButton = (
    action: MediaAction,
    Icon: React.ComponentType<{ className?: string }>,
    label: string,
    big = false,
  ) => (
    <Button
      type="button"
      variant="ghost"
      onClick={() => void act(action)}
      disabled={!state?.running || pending != null}
      aria-label={label}
      title={label}
      className={cn("rounded-full p-0 text-foreground", big ? "size-8" : "size-7")}
    >
      <Icon className={big ? "size-[18px]" : "size-4"} />
    </Button>
  );

  // What the header says on the right. A failed read keeps the last state on
  // screen and says so in words; the raw error went to the log.
  const meta = error
    ? `Couldn\u2019t reach ${appLabel}`
    : state == null
      ? ""
      : !state.running
        ? "Not running"
        : isPlaying
          ? "Playing"
          : state.state === "paused"
            ? "Paused"
            : "Stopped";

  const unreachable = state == null && !!error;

  return (
    <Card
      className={className}
      data-testid="now-playing-card"
      data-media-state={state?.state ?? (unreachable ? "unreachable" : "loading")}
    >
      <CardHeader>
        <CardTitle className="min-w-0 truncate">{appLabel}</CardTitle>
        {meta && (
          <CardAction className="text-[11px] text-muted-foreground" role={error ? "status" : undefined}>
            {meta}
          </CardAction>
        )}
      </CardHeader>

      {state == null && !error ? (
        <CardContent aria-busy="true" aria-label={`Reading ${appLabel}`}>
          <div className="flex items-center gap-3 py-1">
            <Skeleton className="size-11 shrink-0 rounded-[6px]" />
            <div className="min-w-0 flex-1 space-y-2">
              <Skeleton className="h-2.5 w-3/5" />
              <Skeleton className="h-2.5 w-2/5" />
            </div>
          </div>
        </CardContent>
      ) : unreachable || !state?.running ? (
        <CardContent className="flex items-center justify-between gap-3 py-2">
          <div className="min-w-0 text-[13px] text-muted-foreground">
            {unreachable ? `Couldn\u2019t reach ${appLabel}.` : `${appLabel} isn\u2019t open.`}
          </div>
          <Button type="button" size="card" variant="secondary" onClick={() => void openApp()} className="shrink-0">
            Open {appLabel}
          </Button>
        </CardContent>
      ) : (
        <CardContent>
          <div className="flex items-center gap-3">
            <div className="flex size-11 shrink-0 items-center justify-center overflow-hidden rounded-[6px] bg-[var(--fill-3,var(--muted))]">
              {state.artwork_url && state.artwork_url !== brokenArtwork ? (
                <img
                  src={state.artwork_url}
                  alt=""
                  className="size-full object-cover"
                  onError={() => setBrokenArtwork(state.artwork_url)}
                />
              ) : (
                <Music2 className="size-5 text-muted-foreground" />
              )}
            </div>
            <div className="min-w-0 flex-1">
              <div className="truncate text-[13px] font-semibold">{hasTrack ? state.track : "Nothing loaded"}</div>
              {state.artist && (
                <div className="truncate text-[11px] text-muted-foreground">
                  {state.artist}
                  {state.album ? ` \u00b7 ${state.album}` : ""}
                </div>
              )}
            </div>
            <div className="-mr-1 flex shrink-0 items-center">
              {controlButton("previous", SkipBack, "Previous track")}
              {isPlaying ? controlButton("pause", Pause, "Pause", true) : controlButton("play", Play, "Play", true)}
              {controlButton("next", SkipForward, "Next track")}
            </div>
          </div>
          {progress != null && (
            <div className="mt-2">
              <Progress value={progress} className="h-[3px]" aria-label="Playback position" />
              <div className="mt-1 flex justify-between text-[10px] tabular-nums text-muted-foreground">
                <span>{formatTime(position)}</span>
                <span>{formatTime(duration)}</span>
              </div>
            </div>
          )}
        </CardContent>
      )}
    </Card>
  );
}
