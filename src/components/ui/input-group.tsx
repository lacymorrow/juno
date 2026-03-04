"use client";

import * as React from "react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";

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

export type InputGroupButtonProps = React.ComponentProps<typeof Button>;

export const InputGroupButton = React.forwardRef<
  HTMLButtonElement,
  InputGroupButtonProps
>(({ className, ...props }, ref) => {
  return (
    <Button
      ref={ref}
      className={cn("shrink-0", className)}
      {...props}
    />
  );
});
InputGroupButton.displayName = "InputGroupButton";

export type InputGroupTextareaProps = React.ComponentProps<typeof Textarea>;

export const InputGroupTextarea = React.forwardRef<
  HTMLTextAreaElement,
  InputGroupTextareaProps
>(({ className, ...props }, ref) => {
  return (
    <Textarea
      ref={ref}
      className={cn(
        "flex-1 border-0 focus-visible:ring-0 focus-visible:ring-offset-0 resize-none",
        className
      )}
      {...props}
    />
  );
});
InputGroupTextarea.displayName = "InputGroupTextarea";
