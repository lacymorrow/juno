"use client";

import type { ComponentProps } from "react";
import { createContext, useCallback, useContext, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import { CheckIcon, CopyIcon } from "lucide-react";

// --- Context ---

interface EnvironmentVariablesContextValue {
  showValues: boolean;
  setShowValues: (show: boolean) => void;
}

const EnvironmentVariablesContext =
  createContext<EnvironmentVariablesContextValue | null>(null);

function useEnvVarsContext() {
  const ctx = useContext(EnvironmentVariablesContext);
  if (!ctx) {
    throw new Error(
      "EnvironmentVariables sub-components must be used within <EnvironmentVariables>"
    );
  }
  return ctx;
}

// --- Root ---

export type EnvironmentVariablesProps = ComponentProps<"div"> & {
  showValues?: boolean;
  defaultShowValues?: boolean;
  onShowValuesChange?: (show: boolean) => void;
};

export const EnvironmentVariables = ({
  showValues: controlledShowValues,
  defaultShowValues = false,
  onShowValuesChange,
  className,
  children,
  ...props
}: EnvironmentVariablesProps) => {
  const [internalShowValues, setInternalShowValues] =
    useState(defaultShowValues);

  const isControlled = controlledShowValues !== undefined;
  const showValues = isControlled ? controlledShowValues : internalShowValues;

  const setShowValues = useCallback(
    (show: boolean) => {
      if (!isControlled) {
        setInternalShowValues(show);
      }
      onShowValuesChange?.(show);
    },
    [isControlled, onShowValuesChange]
  );

  return (
    <EnvironmentVariablesContext.Provider value={{ showValues, setShowValues }}>
      <div className={cn("space-y-3", className)} {...props}>
        {children}
      </div>
    </EnvironmentVariablesContext.Provider>
  );
};

// --- Header ---

export type EnvironmentVariablesHeaderProps = ComponentProps<"div">;

export const EnvironmentVariablesHeader = ({
  className,
  ...props
}: EnvironmentVariablesHeaderProps) => (
  <div
    className={cn("flex items-center justify-between", className)}
    {...props}
  />
);

// --- Title ---

export type EnvironmentVariablesTitleProps = ComponentProps<"h4">;

export const EnvironmentVariablesTitle = ({
  className,
  ...props
}: EnvironmentVariablesTitleProps) => (
  <h4
    className={cn("text-sm font-medium leading-none", className)}
    {...props}
  />
);

// --- Toggle ---

export type EnvironmentVariablesToggleProps = Omit<
  ComponentProps<typeof Switch>,
  "checked" | "onCheckedChange"
> & {
  label?: string;
};

export const EnvironmentVariablesToggle = ({
  label = "Show values",
  className,
  ...props
}: EnvironmentVariablesToggleProps) => {
  const { showValues, setShowValues } = useEnvVarsContext();
  return (
    <label className={cn("flex items-center gap-2 text-xs text-muted-foreground cursor-pointer", className)}>
      <span>{label}</span>
      <Switch
        checked={showValues}
        onCheckedChange={setShowValues}
        {...props}
      />
    </label>
  );
};

// --- Content ---

export type EnvironmentVariablesContentProps = ComponentProps<"div">;

export const EnvironmentVariablesContent = ({
  className,
  ...props
}: EnvironmentVariablesContentProps) => (
  <div className={cn("space-y-2", className)} {...props} />
);

// --- Single Variable ---

export type EnvironmentVariableProps = Omit<ComponentProps<"div">, "onChange"> & {
  name: string;
  value: string;
  onChange?: (value: string) => void;
  required?: boolean;
};

export const EnvironmentVariable = ({
  name,
  value,
  onChange,
  required,
  className,
  ...props
}: EnvironmentVariableProps) => {
  const { showValues } = useEnvVarsContext();
  const [editing, setEditing] = useState(false);

  return (
    <div
      className={cn(
        "flex items-center gap-3 rounded-md border bg-muted/30 px-3 py-2",
        className
      )}
      {...props}
    >
      <EnvironmentVariableName>
        {name}
        {required && <EnvironmentVariableRequired />}
      </EnvironmentVariableName>

      <div className="flex-1 min-w-0">
        {editing && onChange ? (
          <Input
            type={showValues ? "text" : "password"}
            value={value}
            onChange={(e) => onChange(e.target.value)}
            onBlur={() => setEditing(false)}
            className="h-7 text-xs font-mono"
            autoFocus
          />
        ) : (
          <EnvironmentVariableValue
            value={value}
            onClick={onChange ? () => setEditing(true) : undefined}
          />
        )}
      </div>

      {value && <EnvironmentVariableCopyButton value={value} />}
    </div>
  );
};

// --- Variable Name ---

export type EnvironmentVariableNameProps = ComponentProps<"span">;

export const EnvironmentVariableName = ({
  className,
  ...props
}: EnvironmentVariableNameProps) => (
  <span
    className={cn(
      "shrink-0 text-xs font-mono font-medium text-muted-foreground",
      className
    )}
    {...props}
  />
);

// --- Variable Value ---

export type EnvironmentVariableValueProps = ComponentProps<"span"> & {
  value: string;
};

export const EnvironmentVariableValue = ({
  value,
  className,
  onClick,
  ...props
}: EnvironmentVariableValueProps) => {
  const { showValues } = useEnvVarsContext();
  const masked = value ? "\u2022".repeat(Math.min(value.length, 24)) : "";

  return (
    <span
      className={cn(
        "block truncate text-xs font-mono",
        onClick && "cursor-pointer hover:text-foreground",
        className
      )}
      onClick={onClick}
      {...props}
    >
      {showValues ? value : masked || <span className="text-muted-foreground italic">Not set</span>}
    </span>
  );
};

// --- Copy Button ---

export type EnvironmentVariableCopyButtonProps = Omit<
  ComponentProps<typeof Button>,
  "onClick"
> & {
  value: string;
};

export const EnvironmentVariableCopyButton = ({
  value,
  className,
  ...props
}: EnvironmentVariableCopyButtonProps) => {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard API may fail in some contexts — silently ignore
    }
  }, [value]);

  return (
    <Button
      variant="ghost"
      size="icon-xs"
      className={cn("shrink-0", className)}
      onClick={handleCopy}
      title="Copy to clipboard"
      {...props}
    >
      {copied ? (
        <CheckIcon className="size-3 text-green-600" />
      ) : (
        <CopyIcon className="size-3" />
      )}
    </Button>
  );
};

// --- Required Badge ---

export type EnvironmentVariableRequiredProps = ComponentProps<typeof Badge>;

export const EnvironmentVariableRequired = ({
  className,
  ...props
}: EnvironmentVariableRequiredProps) => (
  <Badge
    variant="outline"
    className={cn("ml-1.5 text-[10px] px-1 py-0", className)}
    {...props}
  >
    required
  </Badge>
);
