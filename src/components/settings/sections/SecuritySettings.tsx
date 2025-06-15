import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Shield } from "lucide-react";
import { PermissionsManager } from "../../PermissionsManager";
import { SettingsSectionProps } from "../types";

export default function SecuritySettings({ settings }: SettingsSectionProps) {
  // Suppress unused parameter warning
  void settings;

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">
          Security & Privacy
        </h3>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Shield size={20} />
              macOS Permissions
            </CardTitle>
            <CardDescription>
              Manage system permissions required for AI computer use features
            </CardDescription>
          </CardHeader>
          <CardContent>
            <PermissionsManager
              variant="compact"
              showHeader={false}
              autoRedirectEnabled={false}
              onRefresh={() => {
                // Trigger any refresh callbacks if needed
                settings.loadAllSettings();
              }}
            />

          </CardContent>
        </Card>
      </div>
    </div>
  );
}
