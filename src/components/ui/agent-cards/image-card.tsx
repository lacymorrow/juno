/**
 * ImageCard: a PNG Juno generated or edited, with Open and Show in Finder.
 *
 * Loaded through the asset protocol so no base64 crosses the IPC boundary.
 * That protocol serves absolute paths only, and only paths inside the scope
 * in `tauri.conf.json`. The image tools write to `~/Pictures/Juno/` and hand
 * the model that tilde path, so two things have to be true for a picture to
 * appear: the tilde is expanded before `convertFileSrc` sees it, and
 * `$PICTURE/Juno/**` is in the asset scope. Neither was (LAC-4204), so every
 * generated image rendered as a broken `<img>`. Both are fixed here.
 *
 * A shadcn Card: the picture (tall ones are capped and contained so they do
 * not take the Island), a caption, and the two actions that matter once the
 * file already exists on disk: Open it, or Show it in Finder. Before and
 * after panes carry words, not just an alt attribute. No hover, no glyph on
 * the buttons.
 */

import { convertFileSrc } from "@tauri-apps/api/core";
import { homeDir } from "@tauri-apps/api/path";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useState } from "react";
import { OpenButton } from "@/components/ui/agent-actions";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";

export interface ImageCardProps {
  path?: string;
  caption?: string;
  before?: string;
  className?: string;
}

const PICTURE = "block max-h-80 w-full object-contain";
const PLACEHOLDER = "block h-40 w-full bg-muted";
const CAPTION = "px-3 py-1 text-[11px] text-muted-foreground";

/**
 * `~/Pictures/Juno/x.png` -> `/Users/you/Pictures/Juno/x.png`.
 *
 * The model writes paths the way people say them. `convertFileSrc` does not
 * expand the tilde; it percent-encodes it, and the asset protocol then looks
 * for a directory literally named `~`.
 */
async function expandHome(path: string): Promise<string> {
  // Only a bare "~" or "~/…". "~someone/x" is that person's home, not a
  // suffix on ours, and we have no business guessing where it is.
  if (!/^~(?=\/|$)/.test(path)) return path;
  const home = (await homeDir()).replace(/\/$/, "");
  return path.replace(/^~/, home);
}

interface Resolved {
  abs: string;
  src: string;
}

/** Absolute path plus asset URL for a path the model may have written with `~/`. */
function useAssetSrc(path?: string): Resolved | null {
  const [resolved, setResolved] = useState<Resolved | null>(null);

  useEffect(() => {
    if (!path) {
      setResolved(null);
      return;
    }
    let live = true;
    expandHome(path)
      .then((abs) => {
        if (live) setResolved({ abs, src: convertFileSrc(abs) });
      })
      .catch((err) => {
        console.error("ImageCard: could not resolve path:", err);
        if (live) setResolved(null);
      });
    return () => {
      live = false;
    };
  }, [path]);

  return resolved;
}

/**
 * A "this one failed to load" flag that clears itself whenever `token`
 * changes, during render rather than in an effect, so the card never paints a
 * frame of the old failure over a new file.
 * https://react.dev/learn/you-might-not-need-an-effect
 */
function useFailureFlag(token: unknown): [boolean, () => void] {
  const [failed, setFailed] = useState(false);
  const [seen, setSeen] = useState(token);

  if (seen !== token) {
    setSeen(token);
    if (failed) setFailed(false);
  }

  const mark = useCallback(() => setFailed(true), []);
  return [failed, mark];
}

/**
 * Show in Finder. The image tools write the PNG before the model ever
 * mentions it, so the file is already saved and a Save button would be a lie
 * about what just happened. This reveals it instead.
 */
function RevealButton({ path }: { path: string }) {
  const [status, setStatus] = useState<"idle" | "error">("idle");

  const handleClick = useCallback(async () => {
    try {
      await revealItemInDir(path);
      setStatus("idle");
    } catch (err) {
      console.error("ImageCard: Failed to reveal:", err);
      setStatus("error");
    }
  }, [path]);

  return (
    <span className="inline-flex flex-col items-end gap-1">
      <Button type="button" size="card" variant="secondary" onClick={handleClick}>
        Show in Finder
      </Button>
      {status === "error" && (
        <span className="text-[11px] text-muted-foreground">Could not show that.</span>
      )}
    </span>
  );
}

interface PictureProps {
  after: Resolved;
  /** Null when there is no before pane, or when it failed to load. */
  beforeImage: Resolved | null;
  label: string;
  caption?: string;
  onError: () => void;
  onBeforeError: () => void;
}

function Picture({ after, beforeImage, label, caption, onError, onBeforeError }: PictureProps) {
  if (!beforeImage) {
    return <img src={after.src} alt={label} className={PICTURE} onError={onError} />;
  }
  return (
    <div className="grid grid-cols-2 gap-px bg-[var(--hair,var(--border))]">
      <figure className="bg-card">
        <img src={beforeImage.src} alt="Before" className={PICTURE} onError={onBeforeError} />
        <figcaption className={CAPTION}>Before</figcaption>
      </figure>
      <figure className="bg-card">
        <img
          src={after.src}
          alt={caption ? `After: ${caption}` : "After"}
          className={PICTURE}
          onError={onError}
        />
        <figcaption className={CAPTION}>After</figcaption>
      </figure>
    </div>
  );
}

export function ImageCard({ path, caption, before, className }: ImageCardProps) {
  const after = useAssetSrc(path);
  const beforeImage = useAssetSrc(before);
  const [missing, markMissing] = useFailureFlag(path);
  const [beforeMissing, markBeforeMissing] = useFailureFlag(before);

  if (!path) return null;

  const label = caption || "Image";
  const fileName = path.split("/").pop() || path;

  let picture: React.ReactNode;
  if (missing) {
    picture = (
      <div className="px-3 py-6 text-[13px] text-muted-foreground">
        That picture is not at {fileName} any more.
      </div>
    );
  } else if (!after) {
    picture = <div className={PLACEHOLDER} aria-hidden="true" />;
  } else {
    // The source of an edit can come from anywhere (~/Desktop, a screenshot)
    // and the asset scope only covers ~/Pictures/Juno. When the before pane
    // cannot load, drop to the single picture rather than show the broken
    // image this card exists to stop showing.
    picture = (
      <Picture
        after={after}
        beforeImage={beforeMissing ? null : beforeImage}
        label={label}
        caption={caption}
        onError={markMissing}
        onBeforeError={markBeforeMissing}
      />
    );
  }

  return (
    <Card className={`overflow-hidden ${className ?? ""}`} data-testid="image-card">
      {picture}
      <div className="flex items-center justify-between gap-3 px-3 py-2.5">
        <div className="min-w-0 truncate text-[13px]">{label}</div>
        <div className="flex shrink-0 items-start gap-2">
          {after && <RevealButton path={after.abs} />}
          <OpenButton path={path} label="Open" primary />
        </div>
      </div>
    </Card>
  );
}
