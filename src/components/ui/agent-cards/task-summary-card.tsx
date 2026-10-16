/**
 * TaskSummaryCard: what a long task did, and how much of it got done.
 *
 * A shadcn Card (header, content, footer) drawn with the Mac tokens: a
 * 16px system-blue circle with a check for done rows, a hairline circle for
 * pending ones, "2 of 3" in the header (or "Done" when everything is), a thin
 * meter only while something is pending, and the card's actions in the footer.
 * Nothing is struck through: finished work reads as finished, not cancelled.
 */

import { Check } from "lucide-react";
import { Actions, CopyButton, OpenButton, QueryButton } from "@/components/ui/agent-actions";
import { Card, CardAction, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { cn } from "@/lib/utils";

export interface TaskItem {
  label: string;
  done?: boolean;
  /** A second, smaller line under the label. */
  detail?: string;
}

/** One footer button. Exactly one of `open`, `query`, `copy` says what it does. */
export interface TaskAction {
  label?: string;
  /** A folder, file or URL to open. */
  open?: string;
  /** A request to send back to the agent. */
  query?: string;
  /** Text to copy. */
  copy?: string;
  primary?: boolean;
}

export interface TaskSummaryCardProps {
  title?: string;
  tasks?: TaskItem[];
  actions?: TaskAction[];
  className?: string;
}

function ActionFor({ action, primary }: { action: TaskAction; primary?: boolean }) {
  if (action.open) {
    const isUrl = /^[a-z][a-z0-9+.-]*:\/\//i.test(action.open);
    return isUrl ? (
      <OpenButton url={action.open} label={action.label} primary={primary} />
    ) : (
      <OpenButton path={action.open} label={action.label} primary={primary} />
    );
  }
  if (action.query) return <QueryButton query={action.query} label={action.label} primary={primary} />;
  if (action.copy) return <CopyButton text={action.copy} label={action.label} primary={primary} />;
  return null;
}

function Marker({ done }: { done?: boolean }) {
  return done ? (
    <span
      aria-hidden="true"
      className="mt-px flex size-4 shrink-0 items-center justify-center rounded-full bg-primary text-primary-foreground"
    >
      <Check className="size-[10px]" strokeWidth={3} />
    </span>
  ) : (
    <span
      aria-hidden="true"
      className="mt-px size-4 shrink-0 rounded-full shadow-[inset_0_0_0_1.25px_var(--label-3,var(--border))]"
    />
  );
}

export function TaskSummaryCard({ title = "Tasks", tasks, actions, className }: TaskSummaryCardProps) {
  const rows = Array.isArray(tasks) ? tasks.filter((t) => t && t.label) : [];
  const buttons = (Array.isArray(actions) ? actions : [])
    .filter((a) => a && (a.open || a.query || a.copy))
    // The primary button sits last, at the right edge of the footer.
    .sort((a, b) => Number(!!a.primary) - Number(!!b.primary));
  const doneCount = rows.filter((t) => t.done).length;
  const allDone = rows.length > 0 && doneCount === rows.length;

  return (
    <Card className={className} data-testid="task-summary-card" data-state={rows.length === 0 ? "empty" : allDone ? "done" : "open"}>
      {rows.length === 0 ? (
        <CardContent className="py-3 text-[13px] text-muted-foreground">Nothing to do.</CardContent>
      ) : (
        <>
          <CardHeader>
            <CardTitle className="min-w-0 truncate">{title}</CardTitle>
            <CardAction className="text-[11px] tabular-nums text-muted-foreground">
              {allDone ? "Done" : `${doneCount} of ${rows.length}`}
            </CardAction>
          </CardHeader>
          <CardContent>
            {!allDone && <Progress value={(doneCount / rows.length) * 100} className="mb-2" aria-label="Progress" />}
            <ul>
              {rows.map((task, i) => (
                <li
                  key={`${task.label}-${i}`}
                  data-done={task.done ? "true" : "false"}
                  className="flex items-start gap-2.5 border-t border-[var(--sep,var(--border))] py-[7px] text-[13px] first:border-t-0"
                >
                  <Marker done={task.done} />
                  <div className="min-w-0 flex-1">
                    <div className={cn("break-words", !task.done && "text-muted-foreground")}>{task.label}</div>
                    {task.detail && <div className="break-words text-[11px] text-muted-foreground">{task.detail}</div>}
                  </div>
                </li>
              ))}
            </ul>
          </CardContent>
        </>
      )}
      {buttons.length > 0 && (
        <CardFooter>
          <Actions>
            {buttons.map((action, i) => (
              <ActionFor key={i} action={action} primary={action.primary} />
            ))}
          </Actions>
        </CardFooter>
      )}
    </Card>
  );
}
