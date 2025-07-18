"use client";

import { useEffect, useRef, useState, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { UI, EVENTS } from "@/lib/constants.generated";
import { safeCleanupEventListener } from "@/lib/safeEventCleanup";

// === STANDARDIZED UI API TYPES ===

/**
 * Backend State Data Structure - Matches exactly what backend emits
 * This structure is defined in ui_commands.rs emit_bar_state_update()
 */
interface BarStateData {
  // Core state
  barState: string;
  inputValue: string;
  lastSubmittedValue: string;
  currentError: string | null;

  // Voice and transcription
  transcriptionText: string;
  spokenText: string;
  voiceMode: string;
  audioLevel: number;

  // Status flags
  isAgentWorking: boolean;
  isDictationMode: boolean;
  isAlwaysListening: boolean;

  // Agent state
  agentState: string | null;
}


// Component-specific AppState type for visualization
export type AppState =
  | typeof UI.AGENT_STATUS_IDLE
  | typeof UI.AGENT_STATUS_LISTENING
  | typeof UI.AGENT_STATUS_PROCESSING
  | typeof UI.AGENT_STATUS_RESPONDING
  | typeof UI.AGENT_STATUS_ERROR
  | typeof UI.AGENT_STATUS_FINISHED
  | typeof UI.AGENT_STATUS_DICTATING
  | typeof UI.AGENT_STATUS_SPEAKING
  | typeof UI.AGENT_STATUS_INPUT
  | typeof UI.AGENT_STATUS_SUCCESS
  | typeof UI.AGENT_STATUS_RESPONSE;

export interface AudioVisualizerProps {
  /** Current application state */
  appState?: AppState;
  /** Width of the canvas in pixels */
  width?: number;
  /** Height of the canvas in pixels */
  height?: number;
  /** Whether to enable microphone input for real audio data */
  enableMicrophone?: boolean;
  /** Custom class name for styling */
  className?: string;
  /** Callback fired when state transitions complete */
  onTransitionComplete?: (newState: AppState) => void;
  /** Animation intensity multiplier (0-2) */
  intensity?: number;
  /** Whether to show transition progress bar */
  showTransitionProgress?: boolean;
  /** Custom colors for each state */
  stateColors?: Partial<Record<AppState, { r: number; g: number; b: number }>>;
  /** Animation style preset */
  animationStyle?: "smooth" | "sharp" | "organic" | "minimal";
}

// === STATE MAPPING ===

/**
 * Maps backend BarState to visualization AppState
 */
const mapBarStateToAppState = (barState: string): AppState => {
  switch (barState) {
    case UI.BAR_STATES_LISTENING:
      return UI.AGENT_STATUS_LISTENING;
    case UI.BAR_STATES_LOADING:
    case UI.BAR_STATES_SUBMITTING:
    case UI.BAR_STATES_TRANSCRIBING:
      return UI.AGENT_STATUS_PROCESSING;
    case UI.BAR_STATES_SPEAKING:
      return UI.AGENT_STATUS_SPEAKING;
    case UI.BAR_STATES_AGENT_RESPONDING:
      return UI.AGENT_STATUS_RESPONDING;
    case UI.BAR_STATES_ERROR:
      return UI.AGENT_STATUS_ERROR;
    case UI.BAR_STATES_SUCCESS:
    case UI.BAR_STATES_FINISHING:
      return UI.AGENT_STATUS_SUCCESS;
    case UI.BAR_STATES_DICTATING:
      return UI.AGENT_STATUS_DICTATING;
    case UI.BAR_STATES_INPUT:
      return UI.AGENT_STATUS_INPUT;
    case UI.BAR_STATES_DEFAULT:
    case UI.BAR_STATES_DICTATION_READY:
    case UI.BAR_STATES_ALWAYS_LISTENING:
    default:
      return UI.AGENT_STATUS_IDLE;
  }
};

export default function AudioVisualizer({
  appState = UI.AGENT_STATUS_IDLE,
  width = 400,
  height = 100,
  enableMicrophone = false,
  className = "",
  onTransitionComplete,
  intensity = 1.0,
  showTransitionProgress = true,
  stateColors,
  animationStyle = "organic",
}: AudioVisualizerProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animationRef = useRef<number>();
  const audioContextRef = useRef<AudioContext>();
  const analyserRef = useRef<AnalyserNode>();
  const dataArrayRef = useRef<Uint8Array>();
  const streamRef = useRef<MediaStream>();

  // Use refs for animation state to avoid re-renders
  const transitionProgressRef = useRef(1);
  const transitionStartTimeRef = useRef(0);
  const previousStateRef = useRef<AppState>(UI.AGENT_STATUS_IDLE);

  const [isActive, setIsActive] = useState(false);
  const [currentState, setCurrentState] = useState<AppState>(
    UI.AGENT_STATUS_IDLE
  );
  const [isTransitioning, setIsTransitioning] = useState(false);
  
  // Backend state integration
  const [backendAudioLevel, setBackendAudioLevel] = useState(0);

  // === BACKEND EVENT INTEGRATION ===
  
  /**
   * Listen to backend BAR_STATE_UPDATE events
   * This syncs the visualizer with the backend state
   */
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      try {
        unlisten = await listen<BarStateData>(
          EVENTS.BAR_STATE_UPDATE,
          (event) => {
            console.log("📊 AudioVisualizer: Received state update:", event.payload);

            const payload = event.payload;
            if (payload && typeof payload === "object" && "barState" in payload) {
              // Map backend state to visualization state
              const mappedState = mapBarStateToAppState(payload.barState);
              
              // Update current state if different
              if (mappedState !== currentState) {
                previousStateRef.current = currentState;
                setCurrentState(mappedState);

                // Start transition
                requestAnimationFrame(() => {
                  transitionProgressRef.current = 0;
                  transitionStartTimeRef.current = Date.now();
                  setIsTransitioning(true);
                });
              }
              
              // Update audio level from backend
              setBackendAudioLevel(payload.audioLevel || 0);
            }
          }
        );

        console.log("✅ AudioVisualizer: Event listener established");
      } catch (error) {
        console.error("❌ AudioVisualizer: Failed to setup event listener:", error);
      }
    };

    setupListener();

    return () => {
      safeCleanupEventListener(unlisten);
      console.log("🔄 AudioVisualizer: Event listener cleaned up");
    };
  }, [currentState]);

  // Update current state when appState prop changes (for direct prop usage)
  useEffect(() => {
    if (appState !== currentState) {
      previousStateRef.current = currentState;
      setCurrentState(appState);

      // Start transition
      requestAnimationFrame(() => {
        transitionProgressRef.current = 0;
        transitionStartTimeRef.current = Date.now();
        setIsTransitioning(true);
      });
    }
  }, [appState, currentState]);

  const startAudioCapture = useCallback(async () => {
    if (!enableMicrophone) return;

    // Clean up any existing audio context first
    if (audioContextRef.current && audioContextRef.current.state !== "closed") {
      audioContextRef.current.close();
    }

    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      streamRef.current = stream;

      const audioContext = new AudioContext();
      const analyser = audioContext.createAnalyser();
      const source = audioContext.createMediaStreamSource(stream);

      analyser.fftSize = 256;
      source.connect(analyser);

      audioContextRef.current = audioContext;
      analyserRef.current = analyser;
      dataArrayRef.current = new Uint8Array(analyser.frequencyBinCount);

      setIsActive(true);
    } catch (error) {
      console.error("Error accessing microphone:", error);
      // Clean up on error
      if (streamRef.current) {
        streamRef.current.getTracks().forEach((track) => track.stop());
        streamRef.current = undefined;
      }
    }
  }, [enableMicrophone]);

  const stopAudioCapture = useCallback(() => {
    if (streamRef.current) {
      streamRef.current.getTracks().forEach((track) => track.stop());
      streamRef.current = undefined;
    }
    if (audioContextRef.current && audioContextRef.current.state !== "closed") {
      audioContextRef.current.close();
    }
    audioContextRef.current = undefined;
    analyserRef.current = undefined;
    dataArrayRef.current = undefined;
    setIsActive(false);
  }, []);

  const generateStatePattern = useCallback(
    (time: number, i: number, centerDistance: number, state: AppState) => {
      const baseAmplitude =
        30 * (1 - Math.pow(centerDistance, 1.2)) * intensity;
      const styleMultiplier =
        animationStyle === "minimal"
          ? 0.5
          : animationStyle === "sharp"
          ? 1.3
          : 1.0;

      switch (state) {
        case UI.AGENT_STATUS_IDLE: {
          // Gentle, slow breathing pattern - calm and waiting
          const breathe = Math.sin(time * 0.4) * 0.3 + 1;
          const flowPhase = time * 0.2;
          const spatialWave =
            Math.sin(i * 0.08 + flowPhase) * 20 * breathe * styleMultiplier;
          return Math.abs(spatialWave) + baseAmplitude * 0.6;
        }

        case UI.AGENT_STATUS_LISTENING: {
          // Active, responsive pattern with more energy
          const pulse = Math.sin(time * 1.2) * 0.6 + 1;
          const flowPhase = time * 0.5;
          const spatialWave1 =
            Math.sin(i * 0.12 + flowPhase) * 35 * pulse * styleMultiplier;
          const spatialWave2 =
            Math.sin(i * 0.08 + flowPhase * 0.7) *
            25 *
            pulse *
            0.8 *
            styleMultiplier;
          return Math.abs(spatialWave1 + spatialWave2) + baseAmplitude * 1.2;
        }

        case UI.AGENT_STATUS_PROCESSING: {
          // Rhythmic, analytical pattern - like thinking
          const process1 = Math.sin(time * 2.5) * 0.4 + 1;
          const process2 = Math.sin(time * 3.7) * 0.3 + 1;
          const process3 = Math.sin(time * 1.8) * 0.25 + 1;
          const flowPhase = time * 0.8;
          const spatialWave1 =
            Math.sin(i * 0.15 + flowPhase) * 28 * process1 * styleMultiplier;
          const spatialWave2 =
            Math.sin(i * 0.09 + flowPhase * 0.6) *
            22 *
            process2 *
            styleMultiplier;
          const spatialWave3 =
            Math.sin(i * 0.06 + flowPhase * 1.3) *
            18 *
            process3 *
            styleMultiplier;
          return (
            Math.abs(spatialWave1 + spatialWave2 + spatialWave3) +
            baseAmplitude * 0.9
          );
        }

        case UI.AGENT_STATUS_RESPONDING: {
          // Dynamic, expressive pattern - like speech cadence
          const speech1 = Math.sin(time * 1.8) * 0.7 + 1;
          const speech2 = Math.sin(time * 2.3) * 0.5 + 1;
          const emphasis = Math.pow(Math.sin(time * 0.7), 2) * 0.4 + 1;
          const flowPhase = time * 0.6;
          const spatialWave1 =
            Math.sin(i * 0.11 + flowPhase) *
            40 *
            speech1 *
            emphasis *
            styleMultiplier;
          const spatialWave2 =
            Math.sin(i * 0.07 + flowPhase * 0.8) *
            30 *
            speech2 *
            emphasis *
            styleMultiplier;
          return Math.abs(spatialWave1 + spatialWave2) + baseAmplitude * 1.1;
        }

        case UI.AGENT_STATUS_ERROR: {
          // Sharp, irregular pattern - distress signal
          const jitter1 = Math.sin(time * 4.2) * 0.6 + 1;
          const jitter2 = Math.sin(time * 5.7) * 0.4 + 1;
          const alarm = Math.sin(time * 2.0) * 0.5 + 1;
          const flowPhase = time * 1.2;
          const spatialWave1 =
            Math.sin(i * 0.18 + flowPhase) *
            35 *
            jitter1 *
            alarm *
            styleMultiplier;
          const spatialWave2 =
            Math.sin(i * 0.13 + flowPhase * 1.3) *
            25 *
            jitter2 *
            alarm *
            styleMultiplier;
          const noise = (Math.random() - 0.5) * 15 * alarm * styleMultiplier;
          return (
            Math.abs(spatialWave1 + spatialWave2 + noise) + baseAmplitude * 0.8
          );
        }

        case UI.AGENT_STATUS_FINISHED: {
          // Celebration pattern - triumphant and flowing
          const triumph1 = Math.sin(time * 1.5) * 0.8 + 1;
          const triumph2 = Math.sin(time * 2.1) * 0.6 + 1;
          const sparkle = Math.sin(time * 3.3) * 0.4 + 1;
          const flowPhase = time * 0.4;
          const spatialWave1 =
            Math.sin(i * 0.09 + flowPhase) *
            45 *
            triumph1 *
            sparkle *
            styleMultiplier;
          const spatialWave2 =
            Math.sin(i * 0.06 + flowPhase * 0.7) *
            35 *
            triumph2 *
            sparkle *
            styleMultiplier;
          const celebration =
            Math.sin(i * 0.04 + flowPhase * 1.5) *
            25 *
            sparkle *
            styleMultiplier;
          return (
            Math.abs(spatialWave1 + spatialWave2 + celebration) +
            baseAmplitude * 1.3
          );
        }

        default:
          return baseAmplitude * 0.5;
      }
    },
    [intensity, animationStyle]
  );

  const getStateLayerConfig = useCallback(
    (state: AppState) => {
      const stateColors = {
        [UI.AGENT_STATUS_IDLE]: { r: 100, g: 116, b: 139 },
        [UI.AGENT_STATUS_LISTENING]: { r: 30, g: 80, b: 190 },
        [UI.AGENT_STATUS_PROCESSING]: { r: 168, g: 85, b: 247 },
        [UI.AGENT_STATUS_RESPONDING]: { r: 34, g: 197, b: 94 },
        [UI.AGENT_STATUS_ERROR]: { r: 239, g: 68, b: 68 },
        [UI.AGENT_STATUS_FINISHED]: { r: 16, g: 185, b: 129 },
        [UI.AGENT_STATUS_DICTATING]: { r: 255, g: 165, b: 0 },
        [UI.AGENT_STATUS_SPEAKING]: { r: 34, g: 197, b: 94 },
        [UI.AGENT_STATUS_INPUT]: { r: 100, g: 116, b: 139 },
        [UI.AGENT_STATUS_SUCCESS]: { r: 16, g: 185, b: 129 },
        [UI.AGENT_STATUS_RESPONSE]: { r: 34, g: 197, b: 94 },
      };

      const defaultColors = {
        idle: { r: 100, g: 116, b: 139 },
        listening: { r: 30, g: 80, b: 190 },
        processing: { r: 168, g: 85, b: 247 },
        responding: { r: 34, g: 197, b: 94 },
        error: { r: 239, g: 68, b: 68 },
        finished: { r: 16, g: 185, b: 129 },
        dictating: { r: 255, g: 165, b: 0 },
        speaking: { r: 34, g: 197, b: 94 },
        input: { r: 100, g: 116, b: 139 },
        success: { r: 16, g: 185, b: 129 },
        response: { r: 34, g: 197, b: 94 },
      };

      const color =
        stateColors[state] ||
        defaultColors[state as keyof typeof defaultColors];

      switch (state) {
        case UI.AGENT_STATUS_IDLE:
          return [
            {
              color,
              opacity: 0.3 * intensity,
              offset: 0,
              scale: 1.2,
              frequency: 0.02,
              complexity: 2,
              depth: 0.6,
            },
            {
              color: { r: color.r + 48, g: color.g + 47, b: color.b + 45 },
              opacity: 0.5 * intensity,
              offset: Math.PI / 12,
              scale: 1.0,
              frequency: 0.025,
              complexity: 2,
              depth: 0.8,
            },
            {
              color: {
                r: Math.min(255, color.r + 103),
                g: Math.min(255, color.g + 97),
                b: Math.min(255, color.b + 86),
              },
              opacity: 0.7 * intensity,
              offset: Math.PI / 8,
              scale: 0.8,
              frequency: 0.03,
              complexity: 1,
              depth: 1.0,
            },
          ];

        case UI.AGENT_STATUS_LISTENING:
          return [
            {
              color,
              opacity: 0.2 * intensity,
              offset: 0,
              scale: 1.4,
              frequency: 0.04,
              complexity: 3,
              depth: 0.4,
            },
            {
              color: { r: color.r + 37, g: color.g + 35, b: color.b + 4 },
              opacity: 0.35 * intensity,
              offset: Math.PI / 16,
              scale: 1.2,
              frequency: 0.05,
              complexity: 3,
              depth: 0.6,
            },
            {
              color: { r: color.r + 88, g: color.g + 67, b: color.b + 3 },
              opacity: 0.5 * intensity,
              offset: Math.PI / 10,
              scale: 1.0,
              frequency: 0.06,
              complexity: 2,
              depth: 0.8,
            },
            {
              color: {
                r: Math.min(255, color.r + 132),
                g: Math.min(255, color.g + 89),
                b: Math.min(255, color.b + 1),
              },
              opacity: 0.7 * intensity,
              offset: Math.PI / 8,
              scale: 0.8,
              frequency: 0.07,
              complexity: 2,
              depth: 1.0,
            },
          ];

        case UI.AGENT_STATUS_PROCESSING:
          return [
            {
              color,
              opacity: 0.15 * intensity,
              offset: 0,
              scale: 1.3,
              frequency: 0.06,
              complexity: 4,
              depth: 0.3,
            },
            {
              color: { r: color.r + 28, g: color.g + 44, b: color.b + 2 },
              opacity: 0.25 * intensity,
              offset: Math.PI / 20,
              scale: 1.1,
              frequency: 0.07,
              complexity: 5,
              depth: 0.5,
            },
            {
              color: { r: color.r + 53, g: color.g + 85, b: color.b + 2 },
              opacity: 0.4 * intensity,
              offset: Math.PI / 14,
              scale: 0.9,
              frequency: 0.08,
              complexity: 4,
              depth: 0.7,
            },
            {
              color: {
                r: Math.min(255, color.r + 75),
                g: Math.min(255, color.g + 122),
                b: Math.min(255, color.b + 3),
              },
              opacity: 0.6 * intensity,
              offset: Math.PI / 10,
              scale: 0.7,
              frequency: 0.09,
              complexity: 3,
              depth: 0.9,
            },
          ];

        case UI.AGENT_STATUS_RESPONDING:
          return [
            {
              color,
              opacity: 0.2 * intensity,
              offset: 0,
              scale: 1.5,
              frequency: 0.05,
              complexity: 3,
              depth: 0.35,
            },
            {
              color: { r: color.r + 40, g: color.g + 25, b: color.b + 34 },
              opacity: 0.35 * intensity,
              offset: Math.PI / 18,
              scale: 1.3,
              frequency: 0.06,
              complexity: 4,
              depth: 0.55,
            },
            {
              color: { r: color.r + 100, g: color.g + 42, b: color.b + 78 },
              opacity: 0.5 * intensity,
              offset: Math.PI / 12,
              scale: 1.1,
              frequency: 0.07,
              complexity: 3,
              depth: 0.75,
            },
            {
              color: {
                r: Math.min(255, color.r + 153),
                g: Math.min(255, color.g + 50),
                b: Math.min(255, color.b + 114),
              },
              opacity: 0.7 * intensity,
              offset: Math.PI / 8,
              scale: 0.9,
              frequency: 0.08,
              complexity: 2,
              depth: 1.0,
            },
          ];

        case UI.AGENT_STATUS_ERROR:
          return [
            {
              color,
              opacity: 0.3 * intensity,
              offset: 0,
              scale: 1.2,
              frequency: 0.08,
              complexity: 2,
              depth: 0.4,
            },
            {
              color: {
                r: Math.min(255, color.r + 9),
                g: color.g + 45,
                b: color.b + 45,
              },
              opacity: 0.5 * intensity,
              offset: Math.PI / 16,
              scale: 1.0,
              frequency: 0.1,
              complexity: 1,
              depth: 0.7,
            },
            {
              color: {
                r: Math.min(255, color.r + 13),
                g: Math.min(255, color.g + 97),
                b: Math.min(255, color.b + 97),
              },
              opacity: 0.7 * intensity,
              offset: Math.PI / 10,
              scale: 0.8,
              frequency: 0.12,
              complexity: 2,
              depth: 1.0,
            },
          ];

        case UI.AGENT_STATUS_FINISHED:
          return [
            {
              color,
              opacity: 0.2 * intensity,
              offset: 0,
              scale: 1.4,
              frequency: 0.03,
              complexity: 3,
              depth: 0.3,
            },
            {
              color: { r: color.r + 36, g: color.g + 26, b: color.b + 24 },
              opacity: 0.35 * intensity,
              offset: Math.PI / 20,
              scale: 1.2,
              frequency: 0.04,
              complexity: 4,
              depth: 0.5,
            },
            {
              color: { r: color.r + 94, g: color.g + 20, b: color.b + 30 },
              opacity: 0.5 * intensity,
              offset: Math.PI / 15,
              scale: 1.0,
              frequency: 0.05,
              complexity: 3,
              depth: 0.7,
            },
            {
              color: {
                r: Math.min(255, color.r + 151),
                g: Math.min(255, color.g + 12),
                b: Math.min(255, color.b + 25),
              },
              opacity: 0.7 * intensity,
              offset: Math.PI / 10,
              scale: 0.8,
              frequency: 0.06,
              complexity: 2,
              depth: 0.9,
            },
          ];

        default:
          return [
            {
              color,
              opacity: 0.5 * intensity,
              offset: 0,
              scale: 1.0,
              frequency: 0.03,
              complexity: 2,
              depth: 1.0,
            },
          ];
      }
    },
    [intensity, stateColors]
  );

  // Smooth interpolation and easing functions
  const easeInOutCubic = (t: number) =>
    t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;

  const getTransitionDuration = useCallback(
    (fromState: AppState, toState: AppState) => {
      // Different transition speeds based on state changes
      const stateSpeeds = {
        [UI.AGENT_STATUS_IDLE]: 1000,
        [UI.AGENT_STATUS_LISTENING]: 800,
        [UI.AGENT_STATUS_PROCESSING]: 1200,
        [UI.AGENT_STATUS_RESPONDING]: 600,
        [UI.AGENT_STATUS_ERROR]: 400,
        [UI.AGENT_STATUS_FINISHED]: 1000,
        [UI.AGENT_STATUS_DICTATING]: 700,
        [UI.AGENT_STATUS_SPEAKING]: 600,
        [UI.AGENT_STATUS_INPUT]: 900,
        [UI.AGENT_STATUS_SUCCESS]: 1000,
        [UI.AGENT_STATUS_RESPONSE]: 600,
      };

      const defaultSpeeds = {
        idle: 1000,
        listening: 800,
        processing: 1200,
        responding: 600,
        error: 400,
        finished: 1000,
        dictating: 700,
        speaking: 600,
        input: 900,
        success: 1000,
        response: 600,
      };

      const fromSpeed =
        stateSpeeds[fromState] ||
        defaultSpeeds[fromState as keyof typeof defaultSpeeds] ||
        1000;
      const toSpeed =
        stateSpeeds[toState] ||
        defaultSpeeds[toState as keyof typeof defaultSpeeds] ||
        1000;

      // Special case transitions
      if (
        fromState === UI.AGENT_STATUS_ERROR ||
        toState === UI.AGENT_STATUS_ERROR
      )
        return 300; // Very fast error states
      if (
        fromState === UI.AGENT_STATUS_FINISHED &&
        toState === UI.AGENT_STATUS_IDLE
      )
        return 2000; // Slow fade from success
      if (
        fromState === UI.AGENT_STATUS_IDLE &&
        toState === UI.AGENT_STATUS_LISTENING
      )
        return 600; // Quick activation
      if (
        fromState === UI.AGENT_STATUS_PROCESSING &&
        toState === UI.AGENT_STATUS_RESPONDING
      )
        return 400; // Quick response

      // Use average of both states
      return (fromSpeed + toSpeed) / 2;
    },
    []
  );

  const getTransitionEasing = useCallback(
    (fromState: AppState, toState: AppState, t: number) => {
      // Different easing for different state transitions
      if (
        fromState === UI.AGENT_STATUS_ERROR ||
        toState === UI.AGENT_STATUS_ERROR
      ) {
        // Sharp, immediate transitions for errors
        return t < 0.3 ? 4 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
      }

      if (fromState === UI.AGENT_STATUS_FINISHED) {
        // Gentle fade out from success
        return Math.sin((t * Math.PI) / 2);
      }

      if (toState === UI.AGENT_STATUS_LISTENING) {
        // Quick ramp up to listening
        return t * t;
      }

      if (toState === UI.AGENT_STATUS_PROCESSING) {
        // Smooth into processing
        return t * t * t * (t * (t * 6 - 15) + 10);
      }

      // Default smooth transition
      return easeInOutCubic(t);
    },
    []
  );

  const animate = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const centerY = height / 2;

    // Update transition progress using refs with smoothing
    if (transitionProgressRef.current < 1) {
      const elapsed = Date.now() - transitionStartTimeRef.current;
      const currentTransitionDuration = getTransitionDuration(
        previousStateRef.current,
        currentState
      );
      const rawProgress = elapsed / currentTransitionDuration;
      const smoothProgress = Math.min(rawProgress, 1);

      // Apply easing for smoother transitions
      const targetProgress = getTransitionEasing(
        previousStateRef.current,
        currentState,
        smoothProgress
      );
      transitionProgressRef.current = targetProgress;

      // Only update React state when transition completes
      if (smoothProgress >= 1 && isTransitioning) {
        transitionProgressRef.current = 1;
        setIsTransitioning(false);
        onTransitionComplete?.(currentState);
      }
    } else {
      transitionProgressRef.current = 1;
    }

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    let audioData: number[] = [];

    // Check if we should use backend audio level
    const useBackendAudio = currentState === UI.AGENT_STATUS_LISTENING && backendAudioLevel > 0;

    if (
      isActive &&
      analyserRef.current &&
      dataArrayRef.current &&
      currentState === UI.AGENT_STATUS_LISTENING &&
      !useBackendAudio
    ) {
      analyserRef.current.getByteFrequencyData(dataArrayRef.current);
      // Enhanced microphone processing for more dramatic but smooth effects
      const rawAudioData = Array.from(dataArrayRef.current).slice(0, 80);

      // Apply smoothing and amplification
      audioData = rawAudioData.map((value, i) => {
        // Amplify the signal for more dramatic effect
        let amplified = value * 2.5 * intensity;

        // Add frequency-based scaling (emphasize mid-range frequencies)
        const freqPosition = i / 80;
        const midRangeBoost = Math.sin(freqPosition * Math.PI) * 1.8 + 1;
        amplified *= midRangeBoost;

        // Apply smoothing based on neighboring values for blend
        const prevValue =
          i > 0 ? rawAudioData[i - 1] * 2.5 * intensity : amplified;
        const nextValue =
          i < rawAudioData.length - 1
            ? rawAudioData[i + 1] * 2.5 * intensity
            : amplified;
        const smoothed = (prevValue + amplified * 2 + nextValue) / 4;

        // Add dynamic range compression for consistency
        const compressed = Math.pow(smoothed / 255, 0.7) * 255;

        // Ensure we don't exceed reasonable bounds
        return Math.min(compressed, 255);
      });
    } else if (useBackendAudio) {
      // Use backend audio level for visualization
      const time = Date.now() * 0.0015;
      const normalizedAudioLevel = backendAudioLevel; // Assume 0-1 range from backend
      
      audioData = Array.from({ length: 80 }, (_, i) => {
        const centerDistance = Math.abs(i - 40) / 40;
        const basePattern = generateStatePattern(time, i, centerDistance, currentState);
        
        // Modulate the pattern with the backend audio level
        const audioModulation = 0.3 + (normalizedAudioLevel * 0.7);
        return basePattern * audioModulation;
      });
    } else {
      // Generate audio data based on current state
      const time = Date.now() * 0.0015;
      audioData = Array.from({ length: 80 }, (_, i) => {
        const centerDistance = Math.abs(i - 40) / 40;
        return generateStatePattern(time, i, centerDistance, currentState);
      });
    }

    const time = Date.now() * 0.0008;

    // Handle transitions by crossfading between two complete renders
    if (transitionProgressRef.current < 1) {
      // Create two separate canvases for crossfade
      const tempCanvas1 = document.createElement("canvas");
      const tempCanvas2 = document.createElement("canvas");
      tempCanvas1.width = width;
      tempCanvas1.height = height;
      tempCanvas2.width = width;
      tempCanvas2.height = height;
      const ctx1 = tempCanvas1.getContext("2d")!;
      const ctx2 = tempCanvas2.getContext("2d")!;

      // Generate audio data for previous state
      const previousAudioData = Array.from({ length: 80 }, (_, i) => {
        const centerDistance = Math.abs(i - 40) / 40;
        return generateStatePattern(
          time,
          i,
          centerDistance,
          previousStateRef.current
        );
      });

      // Render previous state to first canvas
      renderWaveform(
        ctx1,
        width,
        height,
        centerY,
        previousAudioData,
        getStateLayerConfig(previousStateRef.current),
        time,
        previousStateRef.current,
        backendAudioLevel
      );

      // Render current state to second canvas
      renderWaveform(
        ctx2,
        width,
        height,
        centerY,
        audioData,
        getStateLayerConfig(currentState),
        time,
        currentState,
        backendAudioLevel
      );

      // Crossfade between the two renders
      const fadeProgress = transitionProgressRef.current;

      // Draw previous state at full opacity, then current state on top with increasing opacity
      ctx.globalAlpha = 1 - fadeProgress;
      ctx.drawImage(tempCanvas1, 0, 0);

      ctx.globalAlpha = fadeProgress;
      ctx.drawImage(tempCanvas2, 0, 0);

      ctx.globalAlpha = 1; // Reset
    } else {
      // No transition - render current state directly
      renderWaveform(
        ctx,
        width,
        height,
        centerY,
        audioData,
        getStateLayerConfig(currentState),
        time,
        currentState,
        backendAudioLevel
      );
    }

    animationRef.current = requestAnimationFrame(animate);
  }, [
    width,
    height,
    currentState,
    isActive,
    generateStatePattern,
    getStateLayerConfig,
    intensity,
    isTransitioning,
    onTransitionComplete,
    getTransitionDuration,
    getTransitionEasing,
    backendAudioLevel,
  ]);

  // Extract waveform rendering into separate function for reuse
  const renderWaveform = useCallback(
    (
      ctx: CanvasRenderingContext2D,
      width: number,
      height: number,
      centerY: number,
      audioData: number[],
      layers: any[],
      time: number,
      state: AppState,
      audioLevel: number = 0
    ) => {
      // Draw layers
      layers.forEach(
        ({ color, opacity, offset, scale, frequency, complexity, depth }) => {
          const safeDepth = isNaN(depth)
            ? 1.0
            : Math.max(0.1, Math.min(1.0, depth));
          const depthScale = 0.3 + safeDepth * 0.7;
          const depthBlur = (1 - safeDepth) * 6;
          const depthBrightness = 0.6 + safeDepth * 0.4;

          // State-specific intensity modifiers
          let intensityMultiplier = 1.0;
          if (state === UI.AGENT_STATUS_LISTENING) intensityMultiplier = 1.3;
          else if (state === UI.AGENT_STATUS_RESPONDING)
            intensityMultiplier = 1.5;
          else if (state === UI.AGENT_STATUS_PROCESSING)
            intensityMultiplier = 1.1;
          else if (state === UI.AGENT_STATUS_ERROR) intensityMultiplier = 0.9;
          else if (state === UI.AGENT_STATUS_FINISHED)
            intensityMultiplier = 1.2;
          else if (state === UI.AGENT_STATUS_IDLE) intensityMultiplier = 0.7;

          const gradient = ctx.createRadialGradient(
            width / 2,
            centerY,
            0,
            width / 2,
            centerY,
            width * 0.8
          );
          const safeOpacity = isNaN(opacity)
            ? 0.5
            : Math.max(0, Math.min(1, opacity));
          const baseAlpha = safeOpacity * intensityMultiplier * depthBrightness;
          const safeBaseAlpha = isNaN(baseAlpha)
            ? 0.5
            : Math.max(0, Math.min(1, baseAlpha));

          ctx.save();

          if (depth < 0.6) {
            ctx.filter = `blur(${depthBlur}px)`;
          }

          if (depth < 0.4) {
            ctx.globalCompositeOperation = "multiply";
          } else if (depth < 0.7) {
            ctx.globalCompositeOperation = "screen";
          } else {
            ctx.globalCompositeOperation = "source-over";
          }

          // Create gradient stops
          gradient.addColorStop(
            0,
            `rgba(${color.r}, ${color.g}, ${color.b}, 0)`
          );
          gradient.addColorStop(
            0.02,
            `rgba(${color.r}, ${color.g}, ${color.b}, ${safeBaseAlpha * 0.05})`
          );
          gradient.addColorStop(
            0.08,
            `rgba(${color.r}, ${color.g}, ${color.b}, ${safeBaseAlpha * 0.15})`
          );
          gradient.addColorStop(
            0.15,
            `rgba(${color.r}, ${color.g}, ${color.b}, ${safeBaseAlpha * 0.35})`
          );
          gradient.addColorStop(
            0.25,
            `rgba(${color.r}, ${color.g}, ${color.b}, ${safeBaseAlpha * 0.6})`
          );
          gradient.addColorStop(
            0.4,
            `rgba(${color.r}, ${color.g}, ${color.b}, ${safeBaseAlpha * 0.85})`
          );
          gradient.addColorStop(
            0.5,
            `rgba(${color.r}, ${color.g}, ${color.b}, ${safeBaseAlpha})`
          );
          gradient.addColorStop(
            0.6,
            `rgba(${color.r}, ${color.g}, ${color.b}, ${safeBaseAlpha * 0.85})`
          );
          gradient.addColorStop(
            0.75,
            `rgba(${color.r}, ${color.g}, ${color.b}, ${safeBaseAlpha * 0.6})`
          );
          gradient.addColorStop(
            0.85,
            `rgba(${color.r}, ${color.g}, ${color.b}, ${safeBaseAlpha * 0.35})`
          );
          gradient.addColorStop(
            0.92,
            `rgba(${color.r}, ${color.g}, ${color.b}, ${safeBaseAlpha * 0.15})`
          );
          gradient.addColorStop(
            0.98,
            `rgba(${color.r}, ${color.g}, ${color.b}, ${safeBaseAlpha * 0.05})`
          );
          gradient.addColorStop(
            1,
            `rgba(${color.r}, ${color.g}, ${color.b}, 0)`
          );

          ctx.fillStyle = gradient;
          ctx.beginPath();

          const points: { x: number; y: number }[] = [];
          const resolution = 280;

          for (let i = 0; i <= resolution; i++) {
            const x = (i / resolution) * width;
            const dataIndex = Math.floor(
              (i / resolution) * (audioData.length - 1)
            );
            const audioValue = audioData[dataIndex] || 0;

            const centerDistance =
              Math.abs(i - resolution / 2) / (resolution / 2);
            const organicTaper =
              Math.pow(1 - centerDistance, 2.2 + depth * 0.8) *
              (1 + Math.sin(centerDistance * Math.PI) * 0.15 * depth);

            let organicWave = 0;
            const effectiveComplexity = Math.max(
              1,
              Math.floor(complexity * depthScale)
            );

            for (
              let harmonic = 1;
              harmonic <= effectiveComplexity;
              harmonic++
            ) {
              const harmonicFreq =
                frequency * harmonic * (1 + (1 - depth) * 0.3);
              const harmonicAmp =
                (scale * depthScale) / Math.pow(harmonic, 0.8);
              const flowPhase = time * (0.2 + depth * 0.4) * harmonic * 0.6;
              const depthPhase = (1 - depth) * Math.PI * 0.5;
              const spatialPhase =
                offset + depthPhase + i * harmonicFreq + flowPhase;
              const timeModulation =
                Math.sin(time * (0.6 + depth * 0.6)) * (0.2 + depth * 0.2);
              const verticalOscillation =
                Math.sin(time * (1.0 + depth * 0.4) + harmonic * 0.7) *
                (0.15 + depth * 0.15);
              const depthDistortion =
                Math.sin(time * 0.3 + depth * Math.PI) * 0.1 * (1 - depth);

              organicWave +=
                Math.sin(spatialPhase) *
                harmonicAmp *
                (8 + depth * 8) *
                (1 + timeModulation + verticalOscillation + depthDistortion);
            }

            const audioInfluence =
              (isActive || audioLevel > 0) && state === UI.AGENT_STATUS_LISTENING
                ? (audioValue / 255) *
                  height *
                  (0.35 + depth * 0.25) *
                  scale *
                  organicTaper *
                  1.8
                : (audioValue / 255) *
                  height *
                  (0.15 + depth * 0.1) *
                  scale *
                  organicTaper;
            const waveHeight =
              (organicWave + audioInfluence) *
              organicTaper *
              depthScale *
              intensityMultiplier;
            const y = centerY + waveHeight;
            points.push({ x, y });
          }

          // Draw upper wave
          ctx.moveTo(0, centerY);
          for (let i = 0; i < points.length - 2; i += 1) {
            const p0 = points[i];
            const p1 = points[i + 1];
            const p2 = points[i + 2];

            if (i === 0) {
              ctx.lineTo(p0.x, p0.y);
            }

            const cp1x = p0.x + (p1.x - p0.x) * 0.6;
            const cp1y = p0.y + (p1.y - p0.y) * 0.6;
            const cp2x = p1.x - (p2.x - p1.x) * 0.4;
            const cp2y = p1.y - (p2.y - p1.y) * 0.4;

            ctx.bezierCurveTo(cp1x, cp1y, cp2x, cp2y, p1.x, p1.y);
          }

          ctx.lineTo(width, centerY);

          // Draw mirrored lower wave
          for (let i = points.length - 1; i > 1; i -= 1) {
            const p0 = points[i];
            const p1 = points[i - 1];
            const p2 = points[i - 2];

            const mirroredY0 = centerY - (p0.y - centerY);
            const mirroredY1 = centerY - (p1.y - centerY);
            const mirroredY2 = centerY - (p2.y - centerY);

            if (i === points.length - 1) {
              ctx.lineTo(p0.x, mirroredY0);
            }

            const cp1x = p0.x + (p1.x - p0.x) * 0.6;
            const cp1y = mirroredY0 + (mirroredY1 - mirroredY0) * 0.6;
            const cp2x = p1.x - (p2.x - p1.x) * 0.4;
            const cp2y = mirroredY1 - (mirroredY2 - mirroredY1) * 0.4;

            ctx.bezierCurveTo(cp1x, cp1y, cp2x, cp2y, p1.x, mirroredY1);
          }

          ctx.closePath();
          ctx.fill();

          // Add state-specific effects
          if (
            (state === UI.AGENT_STATUS_RESPONDING ||
              state === UI.AGENT_STATUS_FINISHED) &&
            depth > 0.7
          ) {
            ctx.shadowColor = `rgba(${color.r}, ${color.g}, ${color.b}, ${
              0.3 * depth
            })`;
            ctx.shadowBlur = 12 * depth;
            ctx.globalCompositeOperation = "screen";
            ctx.fill();
          }

          ctx.restore();
        }
      );
    },
    []
  );

  // Start animation loop
  useEffect(() => {
    animationRef.current = requestAnimationFrame(animate);
    return () => {
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current);
      }
    };
  }, [animate]);

  // Handle microphone
  useEffect(() => {
    if (
      enableMicrophone &&
      currentState === UI.AGENT_STATUS_LISTENING &&
      !isActive
    ) {
      startAudioCapture();
    } else if (
      !enableMicrophone ||
      currentState !== UI.AGENT_STATUS_LISTENING
    ) {
      stopAudioCapture();
    }
  }, [
    enableMicrophone,
    currentState,
    isActive,
    startAudioCapture,
    stopAudioCapture,
  ]);

  useEffect(() => {
    return () => {
      // Cleanup function
      if (streamRef.current) {
        streamRef.current.getTracks().forEach((track) => track.stop());
      }
      if (
        audioContextRef.current &&
        audioContextRef.current.state !== "closed"
      ) {
        audioContextRef.current.close();
      }
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current);
      }
    };
  }, []);

  return (
    <div className={`relative ${className}`}>
      <canvas
        ref={canvasRef}
        width={width}
        height={height}
        className="rounded-lg"
        style={{
          background: "transparent",
          filter:
            currentState === UI.AGENT_STATUS_LISTENING ||
            currentState === UI.AGENT_STATUS_RESPONDING
              ? "brightness(1.15) saturate(1.3)"
              : "brightness(0.95)",
        }}
      />

      {showTransitionProgress && isTransitioning && (
        <div className="absolute -bottom-2 left-0 right-0 h-1 bg-white/10 rounded-full overflow-hidden">
          <div
            className="h-full bg-gradient-to-r from-cyan-400 to-purple-400 transition-all duration-75 ease-out"
            style={{
              width: `${Math.min(transitionProgressRef.current * 100, 100)}%`,
              transition: "width 0.1s ease-out",
            }}
          />
        </div>
      )}
    </div>
  );
}
