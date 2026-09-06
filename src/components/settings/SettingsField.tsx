import React from 'react';
import { Label } from '@/components/ui/label';
import { cn } from '@/lib/utils';
import { useAdvancedSettings } from './AdvancedSettingsContext';

interface SettingsFieldProps {
  label: string;
  description?: string;
  children: React.ReactNode;
  htmlFor?: string;
  className?: string;
  required?: boolean;
  /** Only render while the advanced-settings toggle is on. */
  advanced?: boolean;
}

export function SettingsField({
  label,
  description,
  children,
  htmlFor,
  className,
  required = false,
  advanced = false,
}: SettingsFieldProps) {
  const { advanced: showAdvanced } = useAdvancedSettings();
  if (advanced && !showAdvanced) return null;

  return (
    <div className={cn('space-y-2', className)}>
      <div className="space-y-1">
        <Label 
          htmlFor={htmlFor} 
          className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70"
        >
          {label}
          {required && <span className="text-destructive ml-1">*</span>}
        </Label>
        {description && (
          <p className="text-xs text-muted-foreground leading-relaxed">
            {description}
          </p>
        )}
      </div>
      {children}
    </div>
  );
}