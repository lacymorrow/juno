/**
 * LinkCard: a page the agent found, with one button that opens it.
 *
 * A single row in a shadcn Card: title, "domain · description" on one
 * truncating line, and a secondary Open button on the right. No favicon is
 * fetched (an agent-chosen URL never loads in the chat window), and there is
 * no hover state because the card itself is not the control. The button only
 * appears for an http or https address.
 */

import { OpenButton } from "@/components/ui/agent-actions";
import { Card, CardContent } from "@/components/ui/card";

export interface LinkCardProps {
  url?: string;
  title?: string;
  description?: string;
  /** Accepted for older answers; never fetched. */
  favicon?: string;
  className?: string;
}

function parse(url: string): { domain: string; openable: boolean } {
  try {
    const u = new URL(url);
    return { domain: u.hostname.replace(/^www\./, ""), openable: u.protocol === "https:" || u.protocol === "http:" };
  } catch {
    return { domain: url, openable: false };
  }
}

export function LinkCard({ url = "", title, description, className }: LinkCardProps) {
  if (!url && !title) return null;
  const { domain, openable } = parse(url);
  const heading = title || domain;
  const line = [title ? domain : null, description].filter(Boolean).join(" · ");

  return (
    <Card className={className} data-testid="link-card">
      <CardContent className="flex items-center gap-3 py-2.5">
        <div className="min-w-0 flex-1">
          <div className="truncate text-[13px] font-semibold">{heading}</div>
          {line && <div className="truncate text-[11px] text-muted-foreground">{line}</div>}
        </div>
        {openable && <OpenButton url={url} label="Open" />}
      </CardContent>
    </Card>
  );
}
