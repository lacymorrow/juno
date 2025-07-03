import { cn } from "@/lib/utils";
import { AlertCircle, Brain, Mic, MicOff, Type, Volume2 } from "lucide-react";
import { useVoiceState } from "@/contexts/VoiceContext";
import { UI } from "@/lib/constants.generated";

interface VoiceStatusIndicatorProps {
  className?: string;
  showText?: boolean;
  variant?: "compact" | "detailed";
}

export function VoiceStatusIndicator({
  className,
  showText = true,
  variant = "detailed",
}: VoiceStatusIndicatorProps) {
  const voiceState = useVoiceState();

  // Get the appropriate icon based on current state
  const getIcon = () => {
    if (voiceState.error) {
      return <AlertCircle className="h-4 w-4 text-red-500" />;
    }

    if (voiceState.isSpeaking) {
      return <Volume2 className="h-4 w-4 text-purple-500" />;
    }

    if (!voiceState.isListening && !voiceState.isTranscribing) {
      return <MicOff className="h-4 w-4 text-muted-foreground" />;
    }

    switch (voiceState.mode) {
      case UI.VOICE_MODES_DICTATION:
        return <Type className="h-4 w-4 text-orange-500" />;
      case UI.VOICE_MODES_AGENT:
        return <Brain className="h-4 w-4 text-blue-500" />;
      default:
        return <Mic className="h-4 w-4 text-green-500" />;
    }
  };

  // Get status text
  const getStatusText = () => {
    if (voiceState.error) {
      return "Voice Error";
    }

    if (voiceState.isSpeaking) {
      return "Speaking";
    }

    if (voiceState.isTranscribing) {
      return voiceState.mode === UI.VOICE_MODES_DICTATION
        ? "Dictating"
        : "Transcribing";
    }

    if (voiceState.isListening) {
      return voiceState.mode === UI.VOICE_MODES_DICTATION
        ? "Hold to Dictate"
        : "Listening";
    }

    return "Voice Ready";
  };

  // Get color classes based on state
  const getColorClasses = () => {
    if (voiceState.error) {
      return "text-red-500 border-red-200 bg-red-50";
    }

    if (voiceState.isSpeaking) {
      return "text-purple-500 border-purple-200 bg-purple-50";
    }

    switch (voiceState.mode) {
      case UI.VOICE_MODES_DICTATION:
        return voiceState.isListening || voiceState.isTranscribing
          ? "text-orange-500 border-orange-200 bg-orange-50"
          : "text-muted-foreground border-muted bg-muted/20";
      case UI.VOICE_MODES_AGENT:
        return voiceState.isListening || voiceState.isTranscribing
          ? "text-blue-500 border-blue-200 bg-blue-50"
          : "text-muted-foreground border-muted bg-muted/20";
      default:
        return "text-muted-foreground border-muted bg-muted/20";
    }
  };

  // Audio level visualization
  const AudioLevelBar = () => {
    if (!voiceState.isListening || voiceState.mode === UI.VOICE_MODES_IDLE)
      return null;

    return (
      <div className="flex items-center gap-1">
        {[...Array(5)].map((_, i) => (
          <div
            key={i}
            className={cn(
              "w-1 rounded-full transition-all duration-150",
              voiceState.audioLevel > (i + 1) * 20
                ? "bg-current h-3"
                : "bg-current/30 h-1"
            )}
          />
        ))}
      </div>
    );
  };

  if (variant === "compact") {
    return (
      <div className={cn("flex items-center gap-2", className)}>
        <div className={cn("relative", getColorClasses())}>
          {getIcon()}
          {(voiceState.isListening || voiceState.isTranscribing) && (
            <div className="absolute -top-1 -right-1 w-2 h-2 bg-current rounded-full animate-pulse" />
          )}
        </div>
        <AudioLevelBar />
      </div>
    );
  }

  return (
    <div
      className={cn(
        "flex items-center gap-3 p-3 rounded-lg border",
        getColorClasses(),
        className
      )}
    >
      <div className="relative">
        {getIcon()}
        {(voiceState.isListening || voiceState.isTranscribing) && (
          <div className="absolute -top-1 -right-1 w-2 h-2 bg-current rounded-full animate-pulse" />
        )}
      </div>

      <div className="flex-1 min-w-0">
        {showText && (
          <div className="font-medium text-sm">{getStatusText()}</div>
        )}

        {voiceState.transcriptionText && (
          <div className="text-xs text-muted-foreground truncate mt-1">
            "{voiceState.transcriptionText}"
          </div>
        )}

        {voiceState.error && (
          <div className="text-xs text-red-600 mt-1">{voiceState.error}</div>
        )}
      </div>

      <AudioLevelBar />

      {/* Mode indicator badge */}
      {voiceState.mode !== UI.VOICE_MODES_IDLE && (
        <div
          className={cn(
            "px-2 py-1 rounded-full text-xs font-medium border",
            voiceState.mode === UI.VOICE_MODES_DICTATION
              ? "bg-orange-100 text-orange-700 border-orange-200"
              : "bg-blue-100 text-blue-700 border-blue-200"
          )}
        >
          {voiceState.mode === UI.VOICE_MODES_DICTATION
            ? "Dictation"
            : "AI Agent"}
        </div>
      )}
    </div>
  );
}
