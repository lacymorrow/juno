import { cn } from "@/lib/utils";
import { AlertCircle, Brain, Mic, MicOff, Type, Volume2 } from "lucide-react";
import { useVoiceState } from "@/contexts/VoiceContext";

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
      case "dictation":
        return <Type className="h-4 w-4 text-orange-500" />;
      case "agent":
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
      return voiceState.mode === "dictation" ? "Dictating" : "Transcribing";
    }

    if (voiceState.isListening) {
      return voiceState.mode === "dictation" ? "Hold to Dictate" : "Listening";
    }

    return "Voice Ready";
  };

  // Get color classes based on state - Enhanced for macOS styling
  const getColorClasses = () => {
    if (voiceState.error) {
      return "text-red-600 border-red-200/50 bg-gradient-to-r from-red-50 to-pink-50 dark:from-red-950/50 dark:to-pink-950/50 dark:border-red-800/50 dark:text-red-400";
    }

    if (voiceState.isSpeaking) {
      return "text-purple-600 border-purple-200/50 bg-gradient-to-r from-purple-50 to-violet-50 dark:from-purple-950/50 dark:to-violet-950/50 dark:border-purple-800/50 dark:text-purple-400";
    }

    switch (voiceState.mode) {
      case "dictation":
        return voiceState.isListening || voiceState.isTranscribing
          ? "text-orange-600 border-orange-200/50 bg-gradient-to-r from-orange-50 to-amber-50 dark:from-orange-950/50 dark:to-amber-950/50 dark:border-orange-800/50 dark:text-orange-400"
          : "text-muted-foreground border-border/30 bg-muted/20 backdrop-blur-sm";
      case "agent":
        return voiceState.isListening || voiceState.isTranscribing
          ? "text-blue-600 border-blue-200/50 bg-gradient-to-r from-blue-50 to-indigo-50 dark:from-blue-950/50 dark:to-indigo-950/50 dark:border-blue-800/50 dark:text-blue-400"
          : "text-muted-foreground border-border/30 bg-muted/20 backdrop-blur-sm";
      default:
        return "text-muted-foreground border-border/30 bg-muted/20 backdrop-blur-sm";
    }
  };

  // Enhanced Audio level visualization with macOS styling
  const AudioLevelBar = () => {
    if (!voiceState.isListening || voiceState.mode === "idle") return null;

    return (
      <div className="flex items-center gap-1">
        {[...Array(5)].map((_, i) => (
          <div
            key={i}
            className={cn(
              "w-1 rounded-full transition-all duration-200 ease-in-out",
              voiceState.audioLevel > (i + 1) * 20
                ? "bg-current h-3 shadow-sm"
                : "bg-current/30 h-1.5"
            )}
            style={{
              animationDelay: `${i * 100}ms`,
            }}
          />
        ))}
      </div>
    );
  };

  if (variant === "compact") {
    return (
      <div className={cn("flex items-center justify-center gap-3 px-3 py-2 rounded-xl border backdrop-blur-sm transition-all duration-300", getColorClasses(), className)}>
        <div className="relative">
          {getIcon()}
          {(voiceState.isListening || voiceState.isTranscribing) && (
            <div className="absolute -top-1 -right-1 w-2 h-2 bg-current rounded-full animate-pulse shadow-sm" />
          )}
        </div>

        {/* Enhanced Audio Level Bar */}
        <AudioLevelBar />

        {/* Status Text for compact mode when active */}
        {(voiceState.isListening || voiceState.isTranscribing || voiceState.isSpeaking) && (
          <span className="text-xs font-medium tracking-wide">
            {getStatusText()}
          </span>
        )}

        {/* Mode indicator badge for compact mode */}
        {voiceState.mode !== "idle" && (voiceState.isListening || voiceState.isTranscribing) && (
          <div
            className={cn(
              "px-2 py-0.5 rounded-full text-xs font-medium border backdrop-blur-sm",
              voiceState.mode === "dictation"
                ? "bg-orange-100/80 text-orange-700 border-orange-200/50 dark:bg-orange-950/50 dark:text-orange-300 dark:border-orange-800/50"
                : "bg-blue-100/80 text-blue-700 border-blue-200/50 dark:bg-blue-950/50 dark:text-blue-300 dark:border-blue-800/50"
            )}
          >
            {voiceState.mode === "dictation" ? "Dict" : "AI"}
          </div>
        )}
      </div>
    );
  }

  return (
    <div
      className={cn(
        "flex items-center gap-4 p-4 rounded-xl border backdrop-blur-sm shadow-sm transition-all duration-300",
        getColorClasses(),
        className
      )}
    >
      <div className="relative">
        {getIcon()}
        {(voiceState.isListening || voiceState.isTranscribing) && (
          <div className="absolute -top-1 -right-1 w-2 h-2 bg-current rounded-full animate-pulse shadow-sm" />
        )}
      </div>

      <div className="flex-1 min-w-0">
        {showText && (
          <div className="font-semibold text-sm tracking-wide">{getStatusText()}</div>
        )}

        {voiceState.transcriptionText && (
          <div className="text-xs text-muted-foreground/80 truncate mt-1 font-medium">
            "{voiceState.transcriptionText}"
          </div>
        )}

        {voiceState.error && (
          <div className="text-xs text-red-600 dark:text-red-400 mt-1 font-medium">{voiceState.error}</div>
        )}
      </div>

      <AudioLevelBar />

      {/* Enhanced Mode indicator badge */}
      {voiceState.mode !== "idle" && (
        <div
          className={cn(
            "px-3 py-1.5 rounded-lg text-xs font-semibold border backdrop-blur-sm shadow-sm transition-all duration-200",
            voiceState.mode === "dictation"
              ? "bg-gradient-to-r from-orange-100 to-amber-100 text-orange-700 border-orange-200/50 dark:from-orange-950/50 dark:to-amber-950/50 dark:text-orange-300 dark:border-orange-800/50"
              : "bg-gradient-to-r from-blue-100 to-indigo-100 text-blue-700 border-blue-200/50 dark:from-blue-950/50 dark:to-indigo-950/50 dark:text-blue-300 dark:border-blue-800/50"
          )}
        >
          {voiceState.mode === "dictation" ? "Dictation Mode" : "AI Agent Mode"}
        </div>
      )}
    </div>
  );
}
