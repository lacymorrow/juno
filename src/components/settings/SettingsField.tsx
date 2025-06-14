import React from 'react';
import { Label } from '@/components/ui/label';
import { cn } from '@/lib/utils';

interface SettingsFieldProps {
  label: string;
  description?: string;
  children: React.ReactNode;
  htmlFor?: string;
  className?: string;
  required?: boolean;
}

export function SettingsField({
  label,
  description,
  children,
  htmlFor,
  className,
  required = false,
}: SettingsFieldProps) {
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