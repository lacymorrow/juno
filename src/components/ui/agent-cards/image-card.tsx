/**
 * ImageCard: a PNG Juno generated or edited, with Open.
 *
 * Loaded through the asset protocol so no base64 crosses the IPC boundary.
 * A shadcn Card: the picture (tall ones are capped and contained so they do
 * not take the Island), a caption, and one secondary Open button that goes
 * through OpenButton, which expands "~/" correctly. Before and after panes
 * carry words, not just an alt attribute. No hover, no glyph on the button.
 */

import { convertFileSrc } from "@tauri-apps/api/core";
import { OpenButton } from "@/components/ui/agent-actions";
import { Card } from "@/components/ui/card";

export interface ImageCardProps {
  path?: string;
  caption?: string;
  before?: string;
  className?: string;
}

const PICTURE = "block max-h-80 w-full object-contain";

export function ImageCard({ path, caption, before, className }: ImageCardProps) {
  if (!path) return null;
  const src = convertFileSrc(path);
  const beforeSrc = before ? convertFileSrc(before) : undefined;

  return (
    <Card className={`overflow-hidden ${className ?? ""}`} data-testid="image-card">
      {beforeSrc ? (
        <div className="grid grid-cols-2 gap-px bg-[var(--hair,var(--border))]">
          <figure className="bg-card">
            <img src={beforeSrc} alt="Before" className={PICTURE} />
            <figcaption className="px-3 py-1 text-[11px] text-muted-foreground">Before</figcaption>
          </figure>
          <figure className="bg-card">
            <img src={src} alt={caption ? `After: ${caption}` : "After"} className={PICTURE} />
            <figcaption className="px-3 py-1 text-[11px] text-muted-foreground">After</figcaption>
          </figure>
        </div>
      ) : (
        <img src={src} alt={caption || "Image"} className={PICTURE} />
      )}
      <div className="flex items-center justify-between gap-3 px-3 py-2.5">
        <div className="min-w-0 truncate text-[13px]">{caption || "Image"}</div>
        <OpenButton path={path} label="Open" />
      </div>
    </Card>
  );
}
