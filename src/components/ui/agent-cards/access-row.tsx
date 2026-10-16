/**
 * AccessRow: one sentence on the left, one primary button on the right.
 *
 * The shape behind "I need Reminders for that." (PermissionAskCard) and the
 * access-off states of AgendaCard and MessageCard. A shadcn Card with the row
 * inside it; the button is the shadcn Button at the card size, system blue.
 */

import type * as React from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { cn } from "@/lib/utils";

export interface AccessRowProps extends Omit<React.ComponentProps<"div">, "children"> {
  /** The sentence. */
  line: string;
  /** The one action's label. Without `onAction` no button is drawn. */
  action?: string;
  onAction?: () => void;
  disabled?: boolean;
}

export function AccessRow({ line, action, onAction, disabled, className, ...rest }: AccessRowProps) {
  return (
    <Card className={className} {...rest}>
      <CardContent className="flex items-center justify-between gap-3 py-3">
        <p className="min-w-0 text-[13px]">{line}</p>
        {onAction ? (
          <Button type="button" size="card" onClick={onAction} disabled={disabled} className={cn("shrink-0")}>
            {action}
          </Button>
        ) : null}
      </CardContent>
    </Card>
  );
}
