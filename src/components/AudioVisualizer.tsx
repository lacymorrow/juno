import { listen } from "@tauri-apps/api/event";
import { useEffect, useState, useRef } from "react";
import { cn } from "@/lib/utils";

interface AudioLevelEvent {
  level: number; // 0-100 normalized level
  db: number; // dB level
}

interface AudioVisualizerProps {
  isActive: boolean;
  className?: string;
  barCount?: number;
  variant?: "bars" | "wave" | "circle";
}

const AudioVisualizer: React.FC<AudioVisualizerProps> = ({
  isActive,
  className,
  barCount = 20,
  variant = "bars"
}) => {
  const [audioLevel, setAudioLevel] = useState(0);
  const [bars, setBars] = useState<number[]>(new Array(barCount).fill(0));
  const animationFrameRef = useRef<number>();
  const lastUpdateTime = useRef(Date.now());

  // Listen for audio level events
  useEffect(() => {
    if (!isActive) {
      setAudioLevel(0);
      setBars(new Array(barCount).fill(0));
      return;
    }

    const setupListener = async () => {
      const unlisten = await listen<AudioLevelEvent>(
        "voice-transcription:audio-level",
        (event) => {
          const { level } = event.payload;
          setAudioLevel(level);
          lastUpdateTime.current = Date.now();
        }
      );

      return unlisten;
    };

    let unlisten: (() => void) | undefined;
    setupListener().then(fn => unlisten = fn);

    return () => {
      unlisten?.();
    };
  }, [isActive, barCount]);

  // Animate bars with decay effect
  useEffect(() => {
    if (!isActive) return;

    const animate = () => {
      const now = Date.now();
      const timeSinceUpdate = now - lastUpdateTime.current;
      
      setBars(prevBars => {
        const newBars = [...prevBars];
        
        // Add new random heights based on current audio level
        for (let i = 0; i < barCount; i++) {
          const targetHeight = audioLevel * (0.5 + Math.random() * 0.5);
          const currentHeight = newBars[i];
          
          // Smooth animation towards target
          if (timeSinceUpdate < 1000) { // Only animate if we've received recent data
            newBars[i] = currentHeight + (targetHeight - currentHeight) * 0.3;
          } else {
            // Decay if no recent audio data
            newBars[i] = Math.max(0, currentHeight * 0.95);
          }
        }
        
        return newBars;
      });

      animationFrameRef.current = requestAnimationFrame(animate);
    };

    animate();

    return () => {
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, [isActive, audioLevel, barCount]);

  if (!isActive) {
    return null;
  }

  const renderBars = () => (
    <div className="flex items-end justify-center gap-1 h-16">
      {bars.map((height, index) => (
        <div
          key={index}
          className="bg-gradient-to-t from-blue-500 to-blue-300 rounded-t-sm transition-all duration-75"
          style={{
            height: `${Math.max(2, height)}%`,
            width: '3px',
            opacity: Math.max(0.3, height / 100),
          }}
        />
      ))}
    </div>
  );

  const renderWave = () => (
    <div className="flex items-center justify-center h-16">
      <svg
        width="200"
        height="64"
        viewBox="0 0 200 64"
        className="overflow-visible"
      >
        <path
          d={`M 0 32 ${bars.map((height, i) => 
            `L ${(i * 200) / barCount} ${32 - (height * 0.3)}`
          ).join(' ')} L 200 32`}
          fill="none"
          stroke="url(#waveGradient)"
          strokeWidth="2"
          className="opacity-80"
        />
        <defs>
          <linearGradient id="waveGradient" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stopColor="#3b82f6" />
            <stop offset="50%" stopColor="#8b5cf6" />
            <stop offset="100%" stopColor="#06b6d4" />
          </linearGradient>
        </defs>
      </svg>
    </div>
  );

  const renderCircle = () => (
    <div className="flex items-center justify-center h-16">
      <div className="relative">
        <div 
          className="w-12 h-12 rounded-full bg-gradient-to-r from-blue-500 to-purple-500 transition-all duration-150"
          style={{
            transform: `scale(${1 + (audioLevel / 200)})`,
            opacity: Math.max(0.5, audioLevel / 100),
          }}
        />
        <div className="absolute inset-0 w-12 h-12 rounded-full border-2 border-blue-300 opacity-50 animate-ping" />
      </div>
    </div>
  );

  return (
    <div className={cn(
      "flex flex-col items-center justify-center p-4 rounded-lg bg-gradient-to-r from-blue-50 to-purple-50 border border-blue-200",
      "shadow-sm transition-all duration-300",
      isActive && "shadow-md border-blue-300",
      className
    )}>
      <div className="flex items-center gap-2 mb-2">
        <div className="w-2 h-2 bg-red-500 rounded-full animate-pulse" />
        <span className="text-sm font-medium text-gray-700">Recording</span>
      </div>
      
      {variant === "bars" && renderBars()}
      {variant === "wave" && renderWave()}
      {variant === "circle" && renderCircle()}
      
      <div className="mt-2 text-xs text-gray-500">
        Level: {Math.round(audioLevel)}%
      </div>
    </div>
  );
};

export default AudioVisualizer;