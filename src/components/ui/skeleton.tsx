import { cn } from "@/lib/utils"

function Skeleton({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="skeleton"
      className={cn("bg-[var(--skeleton,var(--accent))] animate-pulse [animation-duration:var(--skeleton-pulse,2s)] rounded-md", className)}
      {...props}
    />
  )
}

export { Skeleton }
