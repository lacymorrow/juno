import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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
            <CardTitle>Permissions</CardTitle>
            <CardDescription>
              Manage app permissions and privacy settings
            </CardDescription>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-gray-500">
              Permission management features will be available here.
            </p>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}