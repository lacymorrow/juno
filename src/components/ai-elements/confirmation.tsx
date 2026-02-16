"use client";

import type { ComponentProps, ReactNode } from "react";
import { createContext, useContext } from "react";

import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { CheckCircleIcon, XCircleIcon } from "lucide-react";

// Confirmation states align with AI SDK tool part states
type ConfirmationState =
  | "approval-requested"
  | "approval-responded"
  | "output-available"
  | "output-denied";

interface ConfirmationContextValue {
  state: ConfirmationState;
}

const ConfirmationContext = createContext<ConfirmationContextValue | null>(null);

function useConfirmationContext() {
  const ctx = useContext(ConfirmationContext);
  if (!ctx) {
    throw new Error(
      "Confirmation sub-components must be used within <Confirmation>"
    );
  }
  return ctx;
}

// --- Root ---

export type ConfirmationProps = ComponentProps<typeof Alert> & {
  state: ConfirmationState;
};

export const Confirmation = ({
  state,
  className,
  children,
  ...props
}: ConfirmationProps) => (
  <ConfirmationContext.Provider value={{ state }}>
    <Alert
      className={cn(
        "mt-2 border-dashed",
        state === "approval-requested" && "border-yellow-500/50 bg-yellow-50/50 dark:bg-yellow-950/20",
        (state === "approval-responded" || state === "output-available") &&
          "border-green-500/50 bg-green-50/50 dark:bg-green-950/20",
        state === "output-denied" && "border-orange-500/50 bg-orange-50/50 dark:bg-orange-950/20",
        className
      )}
      {...props}
    >
      {children}
    </Alert>
  </ConfirmationContext.Provider>
);

// --- Conditional renderers ---

export type ConfirmationRequestProps = { children: ReactNode };

export const ConfirmationRequest = ({ children }: ConfirmationRequestProps) => {
  const { state } = useConfirmationContext();
  if (state !== "approval-requested") return null;
  return <>{children}</>;
};

export type ConfirmationAcceptedProps = ComponentProps<"div">;

export const ConfirmationAccepted = ({
  className,
  children,
  ...props
}: ConfirmationAcceptedProps) => {
  const { state } = useConfirmationContext();
  if (state !== "approval-responded" && state !== "output-available")
    return null;
  return (
    <div
      className={cn(
        "flex items-center gap-1.5 text-sm text-green-700 dark:text-green-400",
        className
      )}
      {...props}
    >
      <CheckCircleIcon className="size-4" />
      <span>{children}</span>
    </div>
  );
};

export type ConfirmationRejectedProps = ComponentProps<"div">;

export const ConfirmationRejected = ({
  className,
  children,
  ...props
}: ConfirmationRejectedProps) => {
  const { state } = useConfirmationContext();
  if (state !== "output-denied") return null;
  return (
    <div
      className={cn(
        "flex items-center gap-1.5 text-sm text-orange-700 dark:text-orange-400",
        className
      )}
      {...props}
    >
      <XCircleIcon className="size-4" />
      <span>{children}</span>
    </div>
  );
};

// --- Actions container ---

export type ConfirmationActionsProps = ComponentProps<"div">;

export const ConfirmationActions = ({
  className,
  ...props
}: ConfirmationActionsProps) => {
  const { state } = useConfirmationContext();
  if (state !== "approval-requested") return null;
  return (
    <div
      className={cn("flex items-center gap-2", className)}
      {...props}
    />
  );
};

// --- Action button ---

export type ConfirmationActionProps = ComponentProps<typeof Button>;

export const ConfirmationAction = ({
  size = "sm",
  ...props
}: ConfirmationActionProps) => <Button size={size} {...props} />;
