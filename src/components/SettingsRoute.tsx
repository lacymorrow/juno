import "../styles/globals.css"; // Make sure global styles are available
import ModularSettingsWindow from "./settings/ModularSettingsWindow";

export default function SettingsRoute() {
  return (
    <div className="h-screen w-screen bg-background text-foreground">
      <ModularSettingsWindow />
    </div>
  );
}
