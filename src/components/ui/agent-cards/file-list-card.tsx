/**
 * FileListCard: a folder's contents or a result set.
 *
 * A shadcn Card with list rows. The header carries the title and "127 items
 * · 4.2 GB"; the path is a mono caption cut from the left so the folder you
 * are in stays visible. Rows show one trailing value (a folder's count or a
 * file's size). Past eight rows the footer offers "Show all N", which asks the
 * agent for the rest through QueryButton. Nothing here is clickable by hover.
 */

import { Archive, File, FileText, Folder, Image, Music, Video, type LucideIcon } from "lucide-react";
import { QueryButton } from "@/components/ui/agent-actions";
import { Card, CardAction, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { ListRow } from "./list-row";

const FILE_ICONS: Record<string, LucideIcon> = {
  folder: Folder,
  document: FileText,
  image: Image,
  audio: Music,
  video: Video,
  archive: Archive,
  file: File,
};

const MAX_ROWS = 8;

interface FileEntry {
  name: string;
  type?: string;
  size?: string;
  count?: number;
}

export interface FileListCardProps {
  title?: string;
  path?: string;
  files?: FileEntry[];
  totalCount?: number;
  totalSize?: string;
  className?: string;
}

function trailingFor(file: FileEntry): string | undefined {
  if (file.size) return file.size;
  if (typeof file.count === "number") return `${file.count} ${file.count === 1 ? "item" : "items"}`;
  return undefined;
}

export function FileListCard({ title = "Files", path, files, totalCount, totalSize, className }: FileListCardProps) {
  const entries = (Array.isArray(files) ? files : []).filter((f) => f && typeof f.name === "string" && f.name);
  const shown = entries.slice(0, MAX_ROWS);
  const total = typeof totalCount === "number" ? totalCount : entries.length;
  const meta =
    typeof totalCount === "number"
      ? `${totalCount} ${totalCount === 1 ? "item" : "items"}${totalSize ? ` · ${totalSize}` : ""}`
      : undefined;

  return (
    <Card className={className} data-testid="file-list-card" data-state={entries.length === 0 ? "empty" : "ready"}>
      <CardHeader>
        <CardTitle className="min-w-0 truncate">{title}</CardTitle>
        {meta && <CardAction className="text-[11px] tabular-nums text-muted-foreground">{meta}</CardAction>}
      </CardHeader>
      <CardContent>
        {path && (
          // Cut from the left: the leaf of the path is the part that matters.
          <div
            className="truncate font-mono text-[11px] text-muted-foreground"
            style={{ direction: "rtl", textAlign: "left" }}
            title={path}
          >
            <bdi>{path}</bdi>
          </div>
        )}
        {entries.length === 0 ? (
          <div className="py-2 text-[13px] text-muted-foreground">Empty.</div>
        ) : (
          <ul className={path ? "mt-1" : undefined}>
            {shown.map((file, i) => {
              const Icon = FILE_ICONS[(file.type || "file").toLowerCase()] || File;
              return <ListRow key={`${file.name}-${i}`} icon={<Icon />} primary={file.name} trailing={trailingFor(file)} />;
            })}
          </ul>
        )}
      </CardContent>
      {total > shown.length && shown.length > 0 && (
        <CardFooter>
          <QueryButton query={`List all ${total} files in ${path || title}`} label={`Show all ${total}`} />
        </CardFooter>
      )}
    </Card>
  );
}
