import * as React from "react"
import * as ProgressPrimitive from "@radix-ui/react-progress"

import { cn } from "@/lib/utils"

/**
 * shadcn Progress. Under `.agent-content` the Mac tokens make it a 4px meter
 * with a system-blue fill; `tone="warning"` is the one other fill, set by the
 * caller and never computed from the number.
 */
function Progress({
  className,
  value,
  tone,
  ...props
}: React.ComponentProps<typeof ProgressPrimitive.Root> & {
  tone?: "warning" | string
}) {
  return (
    <ProgressPrimitive.Root
      data-slot="progress"
      value={value}
      className={cn(
        "bg-[var(--progress-track,color-mix(in_oklab,var(--primary)_20%,transparent))] relative h-[var(--progress-h,0.5rem)] w-full overflow-hidden rounded-full",
        className
      )}
      {...props}
    >
      <ProgressPrimitive.Indicator
        data-slot="progress-indicator"
        className={cn(
          "h-full w-full flex-1 transition-all",
          tone === "warning" ? "bg-[var(--system-orange,#ff9500)]" : "bg-primary"
        )}
        style={{ transform: `translateX(-${100 - (value || 0)}%)` }}
      />
    </ProgressPrimitive.Root>
  )
}

export { Progress }
