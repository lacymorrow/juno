import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { Activity, AlertCircle, CheckCircle, Clock, Cpu } from "lucide-react";

interface AgentStatus {
  agent_type: string;
  is_available: boolean;
  current_tasks: number;
  total_completed: number;
  success_rate: number;
  average_execution_time: { secs: number; nanos: number };
  capabilities: Array<{
    name: string;
    description: string;
    tool_patterns: string[];
    confidence: number;
  }>;
}

interface OrchestratorStatusReport {
  orchestrator_available: boolean;
  current_tasks: number;
  total_tasks_delegated: number;
  success_rate: number;
  agent_statuses: AgentStatus[];
  active_task_count: number;
}

interface AgentStatusIndicatorProps {
  className?: string;
  compact?: boolean;
}

export function AgentStatusIndicator({ className, compact = false }: AgentStatusIndicatorProps) {
  const [status, setStatus] = useState<OrchestratorStatusReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Known agent limits from the backend code
  const AGENT_LIMITS = {
    Browser: 3,
    Desktop: 5,
    System: 10,
    Orchestrator: 5,
  };

  useEffect(() => {
    const fetchStatus = async () => {
      try {
        setLoading(true);
        const result = await invoke<OrchestratorStatusReport>("get_orchestrator_status");
        setStatus(result);
        setError(null);
      } catch (err) {
        setError(err as string);
        console.error("Failed to fetch agent status:", err);
      } finally {
        setLoading(false);
      }
    };

    // Fetch immediately
    fetchStatus();

    // Set up polling every 2 seconds
    const interval = setInterval(fetchStatus, 2000);

    return () => clearInterval(interval);
  }, []);

  const getAgentStatusIcon = (agent: AgentStatus) => {
    if (!agent.is_available) {
      return <AlertCircle className="h-3 w-3 text-red-400" />;
    }
    if (agent.current_tasks > 0) {
      return <Activity className="h-3 w-3 text-blue-400 animate-pulse" />;
    }
    return <CheckCircle className="h-3 w-3 text-green-400" />;
  };

  const getAgentTypeDisplayName = (type: string) => {
    return type.replace(/([A-Z])/g, ' $1').trim();
  };

  const formatExecutionTime = (time: { secs: number; nanos: number }) => {
    const totalMs = time.secs * 1000 + time.nanos / 1000000;
    if (totalMs < 1000) {
      return `${Math.round(totalMs)}ms`;
    }
    return `${(totalMs / 1000).toFixed(1)}s`;
  };

  if (loading) {
    return (
      <div className={cn("flex items-center gap-2", className)}>
        <Cpu className="h-4 w-4 text-blue-400 animate-pulse" />
        {!compact && <span className="text-xs text-white/60">Loading agent status...</span>}
      </div>
    );
  }

  if (error || !status) {
    return (
      <div className={cn("flex items-center gap-2", className)}>
        <AlertCircle className="h-4 w-4 text-red-400" />
        {!compact && <span className="text-xs text-white/60">Status unavailable</span>}
      </div>
    );
  }

  if (compact) {
    // Compact view - just show overall stats
    const totalCurrent = status.agent_statuses.reduce((sum, agent) => sum + agent.current_tasks, 0);
    const totalLimit = Object.values(AGENT_LIMITS).reduce((sum, limit) => sum + limit, 0);
    
    return (
      <div className={cn("flex items-center gap-1 text-xs", className)}>
        <Activity className="h-3 w-3 text-blue-400" />
        <span className="text-white/70">
          {totalCurrent}/{totalLimit}
        </span>
      </div>
    );
  }

  return (
    <div className={cn("space-y-2", className)}>
      {/* Overall Status */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Cpu className="h-4 w-4 text-blue-400" />
          <span className="text-sm font-medium text-white">Agent System</span>
        </div>
        <div className="flex items-center gap-1 text-xs text-white/60">
          <Clock className="h-3 w-3" />
          <span>{status.active_task_count} active</span>
        </div>
      </div>

      {/* Individual Agent Status */}
      <div className="space-y-1">
        {status.agent_statuses.map((agent) => {
          const limit = AGENT_LIMITS[agent.agent_type as keyof typeof AGENT_LIMITS] || 0;
          const utilizationPercent = limit > 0 ? (agent.current_tasks / limit) * 100 : 0;
          
          return (
            <div key={agent.agent_type} className="flex items-center justify-between text-xs">
              <div className="flex items-center gap-2 flex-1 min-w-0">
                {getAgentStatusIcon(agent)}
                <span className="text-white/80 truncate">
                  {getAgentTypeDisplayName(agent.agent_type)}
                </span>
              </div>
              
              <div className="flex items-center gap-2">
                {/* Call limit indicator */}
                <div className="flex items-center gap-1">
                  <span className="text-white/70">
                    {agent.current_tasks}/{limit}
                  </span>
                  
                  {/* Visual usage bar */}
                  <div className="w-8 h-1 bg-white/20 rounded-full overflow-hidden">
                    <div 
                      className={cn(
                        "h-full rounded-full transition-all duration-300",
                        utilizationPercent === 0 ? "bg-green-400" :
                        utilizationPercent < 50 ? "bg-green-400" :
                        utilizationPercent < 80 ? "bg-yellow-400" :
                        "bg-red-400"
                      )}
                      style={{ width: `${Math.max(utilizationPercent, 3)}%` }}
                    />
                  </div>
                </div>
                
                {/* Success rate */}
                {agent.total_completed > 0 && (
                  <span className="text-white/50">
                    {Math.round(agent.success_rate * 100)}%
                  </span>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {/* Overall System Stats */}
      {status.total_tasks_delegated > 0 && (
        <div className="pt-1 border-t border-white/10">
          <div className="flex items-center justify-between text-xs">
            <span className="text-white/60">Total: {status.total_tasks_delegated}</span>
            <span className="text-white/60">
              Success: {Math.round(status.success_rate * 100)}%
            </span>
          </div>
        </div>
      )}
    </div>
  );
}