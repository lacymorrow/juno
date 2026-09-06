import React from 'react';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { cn } from '@/lib/utils';
import { useAdvancedSettings } from './AdvancedSettingsContext';

interface SettingsSectionProps {
  title: string;
  description?: string;
  icon?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
  loading?: boolean;
  /** Only render while the advanced-settings toggle is on. */
  advanced?: boolean;
}

export function SettingsSection({
  title,
  description,
  icon,
  children,
  className,
  loading = false,
  advanced = false,
}: SettingsSectionProps) {
  const { advanced: showAdvanced } = useAdvancedSettings();
  if (advanced && !showAdvanced) return null;

  return (
    <Card className={cn('relative', className)}>
      <CardHeader className="pb-3">
        <CardTitle className="flex items-center gap-2 text-lg">
          {icon}
          {title}
        </CardTitle>
        {description && (
          <CardDescription className="text-sm text-muted-foreground">
            {description}
          </CardDescription>
        )}
      </CardHeader>
      <CardContent className={cn('space-y-4', loading && 'opacity-50 pointer-events-none')}>
        {children}
      </CardContent>
      {loading && (
        <div className="absolute inset-0 flex items-center justify-center bg-background/50 rounded-lg">
          <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-primary" />
        </div>
      )}
    </Card>
  );
}