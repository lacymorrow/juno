import { useState, useRef, useEffect, type FormEvent } from "react"
import { Mic, Send, Check } from "lucide-react"

type BarState = "default" | "expanding" | "input" | "shrinking" | "loading" | "finishing" | "success"

export function FloatingBar() {
  const [barState, setBarState] = useState<BarState>("default")
  const [inputValue, setInputValue] = useState("")
  const [showTooltip, setShowTooltip] = useState(false)
  const [tooltipVisible, setTooltipVisible] = useState(false)
  const [lastSubmittedValue, setLastSubmittedValue] = useState("")
  const tooltipTimeoutRef = useRef<NodeJS.Timeout | null>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const transitionTimeoutRef = useRef<NodeJS.Timeout | null>(null)
  const loadingTimeoutRef = useRef<NodeJS.Timeout | null>(null)

  // For debugging - log state changes
  useEffect(() => {
    console.log("Bar state changed to:", barState)
  }, [barState])

  const handleMouseEnter = () => {
    if (barState !== "default") return

    tooltipTimeoutRef.current = setTimeout(() => {
      setShowTooltip(true)
      setTimeout(() => setTooltipVisible(true), 50)
    }, 1000)
  }

  const handleMouseLeave = () => {
    if (tooltipTimeoutRef.current) {
      clearTimeout(tooltipTimeoutRef.current)
      tooltipTimeoutRef.current = null
    }
    setTooltipVisible(false)
    setTimeout(() => setShowTooltip(false), 200)
  }

  const handleBarClick = () => {
    if (barState !== "default") return

    // Start expansion animation
    setBarState("expanding")

    // After expansion animation completes, set to input state and focus the input
    transitionTimeoutRef.current = setTimeout(() => {
      setBarState("input")
      // Ensure input is focused after the state change is applied
      setTimeout(() => {
        if (inputRef.current) inputRef.current.focus()
      }, 0)
    }, 300) // Match the CSS transition duration
  }

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault()
    if (!inputValue.trim()) return

    // Store the submitted value to display during transitions
    setLastSubmittedValue(inputValue.trim())

    // First show a brief success state
    setBarState("success")

    // After success animation, start shrinking animation
    transitionTimeoutRef.current = setTimeout(() => {
      setBarState("shrinking")

      // After shrinking animation, show loader with longer duration
      transitionTimeoutRef.current = setTimeout(() => {
        setBarState("loading")
        console.log("Loading state activated")

        // After loading animation, transition to finishing state
        loadingTimeoutRef.current = setTimeout(() => {
          setBarState("finishing")

          // After finishing animation, return to default
          setTimeout(() => {
            setBarState("default")
            setInputValue("")
          }, 300)
        }, 3000) // Increased loading duration for visibility
      }, 300) // Match the CSS transition duration
    }, 600) // Success state duration
  }

  const handleInputBlur = () => {
    // Always shrink when input loses focus, regardless of content
    if (barState === "input") {
      // Start shrinking animation
      setBarState("shrinking")

      // After shrinking animation, return to default
      transitionTimeoutRef.current = setTimeout(() => {
        setBarState("default")
        setInputValue("") // Clear input when shrinking back
      }, 300) // Match the CSS transition duration
    }
  }

  useEffect(() => {
    return () => {
      if (tooltipTimeoutRef.current) clearTimeout(tooltipTimeoutRef.current)
      if (transitionTimeoutRef.current) clearTimeout(transitionTimeoutRef.current)
      if (loadingTimeoutRef.current) clearTimeout(loadingTimeoutRef.current)
    }
  }, [])

  // Determine dimensions based on state
  const getBarStyles = () => {
    switch (barState) {
      case "default":
        return "h-[20px] w-[60px] px-2"
      case "expanding":
        return "h-[40px] w-[240px] px-3"
      case "input":
        return "h-[40px] w-[240px] px-3"
      case "success":
        return "h-[40px] w-[240px] px-3"
      case "shrinking":
        return "h-[20px] w-[60px] px-2"
      case "loading":
        return "h-[20px] w-[60px] px-2"
      case "finishing":
        return "h-[20px] w-[60px] px-2"
      default:
        return "h-[20px] w-[60px] px-2"
    }
  }

  return (
    <div
      className="fixed bottom-6 left-1/2 transform -translate-x-1/2 z-50 group"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
    >
      {/* Tooltip */}
      {showTooltip && barState === "default" && (
        <div
          className={`
            absolute bottom-full mb-2 left-1/2 transform -translate-x-1/2 
            bg-black/90 text-white text-xs rounded-md px-3 py-2 shadow-lg 
            whitespace-nowrap transition-all duration-200 ease-in-out
            ${tooltipVisible ? "opacity-100 translate-y-0" : "opacity-0 translate-y-2"}
          `}
        >
          <div className="flex items-center space-x-3">
            <div className="flex items-center">
              <Mic className="h-3.5 w-3.5 text-emerald-400 mr-1.5" />
              <span>
                <kbd className="px-1.5 py-0.5 text-[11px] font-semibold text-gray-800 bg-gray-100 border border-gray-200 rounded-sm">
                  ⌘ + Space
                </kbd>
                <span className="ml-1.5 text-[11px]">Start/Stop Dictation</span>
              </span>
            </div>
          </div>
          <div className="absolute -bottom-1 left-1/2 transform -translate-x-1/2 w-2 h-2 bg-black/90 rotate-45"></div>
        </div>
      )}

      {/* Universal Bar Container - Always Present */}
      <div
        className={`
          flex items-center justify-center bg-black/90 backdrop-blur-md text-white 
          rounded-full shadow-lg border border-white/20 overflow-hidden
          transition-all duration-300 ease-in-out
          ${getBarStyles()}
          ${barState === "default" ? "cursor-pointer" : ""}
        `}
        onClick={barState === "default" ? handleBarClick : undefined}
      >
        {/* Default State Content */}
        {(barState === "default" || barState === "finishing") && (
          <div
            className={`
              w-5 h-[4px] bg-emerald-400 rounded-full 
              transition-all duration-300 ease-in-out 
              group-hover:w-8 group-hover:bg-emerald-300
              ${barState === "finishing" ? "opacity-0 animate-fade-in" : ""}
            `}
          ></div>
        )}

        {/* Expanding/Input State Content */}
        {(barState === "expanding" || barState === "input") && (
          <form
            onSubmit={handleSubmit}
            className={`
              flex items-center justify-between w-full h-full
              transition-opacity duration-300 ease-in-out
              ${barState === "input" ? "opacity-100" : "opacity-0"}
            `}
          >
            <input
              ref={inputRef}
              type="text"
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              onBlur={handleInputBlur}
              placeholder="Type and press Enter..."
              className="flex-1 bg-transparent border-none outline-none text-sm text-white placeholder-white/50"
              disabled={barState !== "input"}
            />
            <button
              type="submit"
              className="flex items-center justify-center h-6 w-6 rounded-full bg-emerald-500 hover:bg-emerald-400 transition-colors duration-200"
              disabled={barState !== "input"}
            >
              <Send size={12} className="text-black" />
            </button>
          </form>
        )}

        {/* Success State Content */}
        {barState === "success" && (
          <div className="flex items-center justify-between w-full h-full animate-success-fade">
            <span className="flex-1 overflow-hidden text-ellipsis whitespace-nowrap pl-2 text-sm text-emerald-400 font-medium">
              {lastSubmittedValue}
            </span>
            <div className="flex items-center justify-center h-6 w-6 rounded-full bg-emerald-500">
              <Check size={12} className="text-black" />
            </div>
          </div>
        )}

        {/* Shrinking State - Empty to create clean transition */}
        {barState === "shrinking" && <div className="opacity-0 w-full h-full"></div>}

        {/* Loading State Content */}
        {barState === "loading" && (
          <div className="gooey-container flex items-center justify-center w-full h-full">
            <div className="gooey-dot bg-emerald-400"></div>
            <div className="gooey-dot bg-emerald-400"></div>
            <div className="gooey-dot bg-emerald-400"></div>
          </div>
        )}
      </div>
    </div>
  )
}
