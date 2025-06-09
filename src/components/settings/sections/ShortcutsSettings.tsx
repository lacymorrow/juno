import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { SettingsSectionProps } from "../types";

export default function ShortcutsSettings({ settings }: SettingsSectionProps) {
  // Suppress unused parameter warning
  void settings;
  
  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">
          Keyboard Shortcuts
        </h3>

        <Card>
          <CardHeader>
            <CardTitle>Shortcuts</CardTitle>
            <CardDescription>
              Customize keyboard shortcuts for quick actions
            </CardDescription>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-gray-500">
              Keyboard shortcut configuration will be available here.
            </p>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}