import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Mic } from "lucide-react";
import { UI } from "@/lib/constants.generated";

export interface FloatingBarState {
  isVisible: boolean;
  voiceMode:
    | typeof UI.VOICE_MODES_IDLE
    | typeof UI.VOICE_MODES_AGENT
    | typeof UI.VOICE_MODES_DICTATION;
}

const FloatingBar: React.FC = () => {
  const [barState, setBarState] = useState<FloatingBarState>({
    isVisible: false,
    voiceMode: UI.VOICE_MODES_IDLE,
  });

  useEffect(() => {
    const unlisten = listen<FloatingBarState>("floating-bar-state", (event) => {
      setBarState(event.payload);
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const handleToggle = async () => {
    try {
      await invoke("floating_bar_toggle");
    } catch (error) {
      console.error("Failed to toggle floating bar:", error);
    }
  };

  if (!barState.isVisible) {
    return null;
  }

  return (
    <div className="floating-bar">
      <button onClick={handleToggle} className="voice-toggle">
        <Mic size={24} />
      </button>
    </div>
  );
};

export default FloatingBar;
