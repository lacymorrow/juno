import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { SettingsSectionProps } from "../types";

export default function ToolsSettings({ settings }: SettingsSectionProps) {
  // Suppress unused parameter warning
  void settings;
  
  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">Tools</h3>

        <Card>
          <CardHeader>
            <CardTitle>Tool Configuration</CardTitle>
            <CardDescription>
              Configure available tools and features for the AI agent
            </CardDescription>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-gray-500">
              Tool configuration settings will be available here.
            </p>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}