"use client";

import * as React from "react";
import { cn } from "@/lib/utils";

export interface ButtonGroupProps extends React.HTMLAttributes<HTMLDivElement> {}

export const ButtonGroup = React.forwardRef<HTMLDivElement, ButtonGroupProps>(
  ({ className, children, ...props }, ref) => {
    return (
      <div
        ref={ref}
        className={cn(
          "flex items-center rounded-md border",
          "[&>button]:rounded-none [&>button]:border-0 [&>button]:border-r [&>button:last-child]:border-r-0",
          "[&>button:first-child]:rounded-l-md [&>button:last-child]:rounded-r-md",
          className
        )}
        {...props}
      >
        {children}
      </div>
    );
  }
);
ButtonGroup.displayName = "ButtonGroup";

export interface ButtonGroupTextProps
  extends React.HTMLAttributes<HTMLSpanElement> {}

export const ButtonGroupText = React.forwardRef<
  HTMLSpanElement,
  ButtonGroupTextProps
>(({ className, ...props }, ref) => {
  return (
    <span
      ref={ref}
      className={cn(
        "flex items-center justify-center px-3 text-sm text-muted-foreground",
        className
      )}
      {...props}
    />
  );
});
ButtonGroupText.displayName = "ButtonGroupText";
