"use client"

import * as React from "react"
import { motion, AnimatePresence } from "motion/react"
import { Mic, Square, X } from "lucide-react"

import { cn } from "@/lib/utils"
import { VoiceButton, type VoiceButtonState } from "@/components/ui/voice-button"
import { ShimmeringText } from "@/components/ui/shimmering-text"

export interface VoiceControlBarProps extends React.HTMLAttributes<HTMLDivElement> {
  /**
   * Current state of the voice interaction
   */
  state?: VoiceButtonState
  /**
   * Text to display in the shimmering area (status/feedback)
   */
  statusText?: string
  /**
   * Callback when the voice button is pressed
   */
  onVoicePress?: () => void
  /**
   * Callback to cancel/close the bar
   */
  onClose?: () => void
  /**
   * Whether to show the close button
   */
  showClose?: boolean
}

export function VoiceControlBar({
  state = "idle",
  statusText,
  onVoicePress,
  onClose,
  showClose = true,
  className,
  ...props
}: VoiceControlBarProps) {
  const isRecording = state === "recording"
  const isProcessing = state === "processing"
  
  // Determine text based on state if not provided
  const displayText = React.useMemo(() => {
    if (statusText) return statusText
    
    switch (state) {
      case "idle":
        return "Press to speak..."
      case "recording":
        return "Listening..."
      case "processing":
        return "Thinking..."
      case "success":
        return "Done!"
      case "error":
        return "Something went wrong"
      default:
        return "Ready"
    }
  }, [state, statusText])

  return (
    <motion.div
      initial={{ opacity: 0, y: 20, scale: 0.95 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: 10, scale: 0.95 }}
      transition={{ duration: 0.2, type: "spring", stiffness: 300, damping: 25 }}
      className={cn(
        "relative flex items-center gap-3 rounded-full border bg-background/95 p-2 shadow-xl backdrop-blur-md transition-all duration-300",
        "hover:border-primary/20 hover:shadow-2xl",
        isRecording && "border-primary/30 shadow-primary/10",
        className
      )}
      {...props}
    >
      <div className="flex items-center gap-3 pl-1">
        <VoiceButton
          state={state}
          onPress={onVoicePress}
          size="icon"
          className="h-10 w-10 shrink-0 rounded-full"
          icon={<Mic className="size-5" />}
          waveformClassName="rounded-full"
        />

        <div className="flex min-w-[120px] flex-col justify-center overflow-hidden px-1">
          <AnimatePresence mode="wait">
            <motion.div
              key={state + displayText}
              initial={{ opacity: 0, y: 5 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -5 }}
              transition={{ duration: 0.2 }}
            >
              {isProcessing || isRecording ? (
                <ShimmeringText
                  text={displayText}
                  className="text-sm font-medium"
                  shimmerColor={isRecording ? "var(--primary)" : undefined}
                />
              ) : (
                <span className="text-muted-foreground text-sm font-medium">
                  {displayText}
                </span>
              )}
            </motion.div>
          </AnimatePresence>
        </div>
      </div>

      {showClose && (
        <div className="flex items-center border-l pl-2">
          <button
            onClick={onClose}
            className="text-muted-foreground hover:bg-muted hover:text-foreground inline-flex h-8 w-8 items-center justify-center rounded-full transition-colors"
            aria-label="Close"
          >
            <X className="size-4" />
          </button>
        </div>
      )}
    </motion.div>
  )
}

