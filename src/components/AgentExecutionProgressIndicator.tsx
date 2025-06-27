import { cn } from "@/lib/utils";
import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  AlertCircle,
  CheckCircle,
  Clock,
  Play,
  Square,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

interface AgentExecutionProgress {
  is_executing: boolean;
  execution_id: string | null;
  current_step: number | null;
  max_steps: number | null;
  remaining_steps: number | null;
  progress_percentage: number | null;
}

interface AgentExecutionProgressIndicatorProps {
  className?: string;
  compact?: boolean;
  showProgressBar?: boolean;
}

export function AgentExecutionProgressIndicator({
  className,
  compact = false,
  showProgressBar = true,
}: AgentExecutionProgressIndicatorProps) {
  const [progress, setProgress] = useState<AgentExecutionProgress | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const isInitialLoadRef = useRef(true);

  useEffect(() => {
    const fetchProgress = async () => {
      try {
        // Only show loading state on initial load, not during polling
        if (isInitialLoadRef.current) {
          setLoading(true);
        }

        const result = await invoke<AgentExecutionProgress>(
          "get_agent_execution_progress"
        );
        setProgress(result);
        setError(null);

        if (isInitialLoadRef.current) {
          isInitialLoadRef.current = false;
          setLoading(false);
        }
      } catch (err) {
        setError(err as string);
        console.error("Failed to fetch agent execution progress:", err);
        if (isInitialLoadRef.current) {
          isInitialLoadRef.current = false;
          setLoading(false);
        }
      }
    };

    // Fetch immediately
    fetchProgress();

    // Set up polling every 1 second when executing, every 5 seconds when not
    const pollInterval = progress?.is_executing ? 1000 : 5000;
    const interval = setInterval(() => {
      // Don't change loading state during polling - this prevents flashing
      fetchProgress();
    }, pollInterval);

    return () => clearInterval(interval);
  }, [progress?.is_executing]);

  const getStatusIcon = () => {
    if (loading) {
      return <Clock className="h-3 w-3 text-blue-400 animate-pulse" />;
    }

    if (error || !progress) {
      return <AlertCircle className="h-3 w-3 text-red-400" />;
    }

    if (progress.is_executing) {
      return <Activity className="h-3 w-3 text-blue-400 animate-pulse" />;
    }

    return <CheckCircle className="h-3 w-3 text-green-400" />;
  };

  const getStepText = () => {
    if (!progress || !progress.is_executing) {
      return "Ready";
    }

    if (progress.current_step && progress.max_steps) {
      return `Step ${progress.current_step}/${progress.max_steps}`;
    }

    if (progress.max_steps && progress.remaining_steps) {
      return `${progress.remaining_steps} steps remaining`;
    }

    if (progress.max_steps) {
      return `Max ${progress.max_steps} steps`;
    }

    return "Executing...";
  };

  const getRemainingText = () => {
    if (!progress?.is_executing || !progress.remaining_steps) {
      return null;
    }

    return `${progress.remaining_steps} left`;
  };

  const getProgressPercentage = () => {
    if (progress?.progress_percentage) {
      return progress.progress_percentage;
    }

    if (progress?.current_step && progress?.max_steps) {
      return (progress.current_step / progress.max_steps) * 100;
    }

    return 0;
  };

  if (loading && compact) {
    return (
      <div className={cn("flex items-center gap-1 text-xs", className)}>
        <Clock className="h-3 w-3 text-blue-400 animate-pulse" />
        <span className="text-primary/60">...</span>
      </div>
    );
  }

  if (error && compact) {
    return (
      <div className={cn("flex items-center gap-1 text-xs", className)}>
        <AlertCircle className="h-3 w-3 text-red-400" />
        <span className="text-primary/60">Error</span>
      </div>
    );
  }

  if (compact) {
    // Compact view - show basic execution status
    return (
      <div className={cn("flex items-center gap-1 text-xs", className)}>
        {getStatusIcon()}
        <span className="text-primary/70">{getStepText()}</span>
        {getRemainingText() && (
          <span className="text-primary/50 text-[10px]">
            ({getRemainingText()})
          </span>
        )}
      </div>
    );
  }

  return (
    <div className={cn("space-y-2", className)}>
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          {progress?.is_executing ? (
            <Play className="h-4 w-4 text-blue-400" />
          ) : (
            <Square className="h-4 w-4 text-gray-400" />
          )}
          <span className="text-sm font-medium text-primary">
            Agent Execution
          </span>
        </div>
        <div className="flex items-center gap-1 text-xs text-primary/60">
          {getStatusIcon()}
          <span>{getStepText()}</span>
        </div>
      </div>

      {/* Progress Details */}
      {progress?.is_executing && (
        <div className="space-y-2">
          {/* Progress Bar */}
          {showProgressBar && progress.max_steps && (
            <div className="space-y-1">
              <div className="flex justify-between text-xs text-primary/60">
                <span>Progress</span>
                <span>
                  {progress.current_step || 0}/{progress.max_steps}
                </span>
              </div>
              <div className="w-full h-2 bg-white/20 rounded-full overflow-hidden">
                <div
                  className={cn(
                    "h-full rounded-full transition-all duration-300",
                    getProgressPercentage() < 70
                      ? "bg-blue-400"
                      : getProgressPercentage() < 90
                      ? "bg-yellow-400"
                      : "bg-red-400"
                  )}
                  style={{ width: `${Math.max(getProgressPercentage(), 2)}%` }}
                />
              </div>
            </div>
          )}

          {/* Remaining Steps Warning */}
          {progress.remaining_steps && progress.remaining_steps <= 3 && (
            <div className="flex items-center gap-2 text-xs text-yellow-400">
              <AlertCircle className="h-3 w-3" />
              <span>
                {progress.remaining_steps === 1
                  ? "Last step before cutoff"
                  : `Only ${progress.remaining_steps} steps remaining`}
              </span>
            </div>
          )}

          {/* Execution ID */}
          {progress.execution_id && (
            <div className="text-xs text-primary/40">
              ID: {progress.execution_id.substring(0, 8)}...
            </div>
          )}
        </div>
      )}

      {/* Status Messages */}
      {loading && (
        <div className="text-xs text-primary/60">
          Loading execution status...
        </div>
      )}

      {error && (
        <div className="text-xs text-red-400">
          Failed to load execution status
        </div>
      )}

      {!progress?.is_executing && !loading && !error && (
        <div className="text-xs text-primary/60">
          Agent ready to execute (limit: {progress?.max_steps || 15} steps)
        </div>
      )}
    </div>
  );
}
