import "../styles/globals.css"; // Make sure global styles are available
import { SettingsWindow } from "./settings";

export default function SettingsRoute() {
  return (
    <div className="h-screen w-screen bg-background text-foreground">
      <SettingsWindow />
    </div>
  );
}
