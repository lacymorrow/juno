import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MessageSquare, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

/** Mirrors the Rust `ConversationMeta` (serde snake_case). */
interface ConversationMeta {
  id: string;
  title: string;
  created_at: number;
  updated_at: number;
  message_count: number;
}

/** "3m ago" / "2h ago" / "5d ago" / a date for anything older. */
function relativeTime(unixSecs: number): string {
  const diff = Math.max(0, Date.now() / 1000 - unixSecs);
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
  return new Date(unixSecs * 1000).toLocaleDateString();
}

interface HistoryViewProps {
  /** Load a past conversation and return to the chat view. */
  onLoad: (id: string) => void;
}

/**
 * The list of past conversations. One primary action per row: click the row to
 * reopen that conversation; the trash button deletes it. Restart always starts a
 * new chat, so this is the only way back into an earlier one.
 */
export function HistoryView({ onLoad }: HistoryViewProps) {
  const [items, setItems] = useState<ConversationMeta[]>([]);
  const [currentId, setCurrentId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const [list, active] = await Promise.all([
        invoke<ConversationMeta[]>("list_conversations"),
        invoke<string>("get_current_conversation_id").catch(() => null),
      ]);
      setItems(list);
      setCurrentId(active);
    } catch (err) {
      console.error("Failed to list conversations:", err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleDelete = useCallback(
    async (e: React.MouseEvent, id: string) => {
      e.stopPropagation();
      try {
        await invoke("delete_conversation", { id });
        await refresh();
      } catch (err) {
        console.error("Failed to delete conversation:", err);
      }
    },
    [refresh],
  );

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        Loading history…
      </div>
    );
  }

  if (items.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 text-center text-muted-foreground">
        <MessageSquare size={28} className="opacity-40" />
        <p className="text-sm">No past conversations yet</p>
        <p className="text-xs opacity-70">
          Chats you have are saved here automatically.
        </p>
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto px-2 py-2">
      <ul className="flex flex-col gap-0.5">
        {items.map((item) => {
          const isCurrent = item.id === currentId;
          return (
            <li key={item.id}>
              <button
                type="button"
                onClick={() => onLoad(item.id)}
                className={cn(
                  "group flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left transition-colors",
                  "hover:bg-accent/60",
                  isCurrent && "bg-accent",
                )}
              >
                <MessageSquare
                  size={14}
                  className="shrink-0 text-muted-foreground"
                />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm text-foreground/90">
                    {item.title}
                  </div>
                  <div className="text-xs text-muted-foreground">
                    {relativeTime(item.updated_at)} · {item.message_count} message
                    {item.message_count === 1 ? "" : "s"}
                  </div>
                </div>
                <Button
                  asChild
                  variant="ghost"
                  size="sm"
                  onClick={(e) => handleDelete(e, item.id)}
                  title="Delete conversation"
                  className="h-7 w-7 shrink-0 p-0 opacity-0 group-hover:opacity-100"
                >
                  <span>
                    <Trash2 size={13} className="text-muted-foreground" />
                  </span>
                </Button>
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
