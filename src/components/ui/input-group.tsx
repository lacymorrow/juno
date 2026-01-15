"use client";

import * as React from "react";
import { cn } from "@/lib/utils";

export interface InputGroupProps extends React.HTMLAttributes<HTMLDivElement> {}

export const InputGroup = React.forwardRef<HTMLDivElement, InputGroupProps>(
  ({ className, children, ...props }, ref) => {
    return (
      <div
        ref={ref}
        className={cn(
          "flex items-center rounded-md border bg-background",
          "focus-within:ring-2 focus-within:ring-ring focus-within:ring-offset-2",
          "[&>input]:border-0 [&>input]:focus-visible:ring-0 [&>input]:focus-visible:ring-offset-0",
          className
        )}
        {...props}
      >
        {children}
      </div>
    );
  }
);
InputGroup.displayName = "InputGroup";

export interface InputGroupAddonProps
  extends React.HTMLAttributes<HTMLDivElement> {}

export const InputGroupAddon = React.forwardRef<
  HTMLDivElement,
  InputGroupAddonProps
>(({ className, ...props }, ref) => {
  return (
    <div
      ref={ref}
      className={cn(
        "flex items-center justify-center px-3 text-sm text-muted-foreground",
        className
      )}
      {...props}
    />
  );
});
InputGroupAddon.displayName = "InputGroupAddon";
