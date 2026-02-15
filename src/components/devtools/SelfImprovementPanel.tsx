import React, { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "../ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "../ui/card";
import { Badge } from "../ui/badge";
import { Separator } from "../ui/separator";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "../ui/tabs";
import { Progress } from "../ui/progress";
import { Alert, AlertDescription, AlertTitle } from "../ui/alert";
import { Textarea } from "../ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../ui/select";
import {
  Play,
  Square,
  RefreshCw,
  BarChart3,
  Archive,
  Settings,
  Zap,
  Brain,
  Shield,
  AlertTriangle,
  CheckCircle,
  Info,
  TrendingUp,
  Activity,
  Gauge,
  Target,
  Clock,
  Database,
} from "lucide-react";

interface SelfImprovementStatus {
  is_active: boolean;
  cycles_completed: number;
  success_rate: number;
  performance_gain: number;
  active_agents: string[];
  memory_usage: number;
  last_update: string;
}

interface ImprovementIteration {
  id: string;
  timestamp: string;
  duration_minutes: number;
  status: string;
  performance_gain: number;
  improvements_applied: number;
  safety_checks_passed: number;
}

interface SystemHealth {
  overall_score: number;
  components?: { [key: string]: string } | null;
  vital_signs?: { [key: string]: string } | null;
  recommendations?: string[] | null;
}

interface BenchmarkResult {
  benchmark_type: string;
  score: number;
  target: number;
  status: "passed" | "failed" | "warning";
  details?: string;
}

interface InitializeSelfImprovementRequest {
  config: any | null; // Configuration for the self-improvement system
  load_existing_archive: boolean; // Whether to load existing archive
}

const DEVELOPMENT_MODE_ONLY =
  !process.env.NODE_ENV || process.env.NODE_ENV === "development";

const SelfImprovementPanel: React.FC = () => {
  const [status, setStatus] = useState<SelfImprovementStatus | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState("overview");
  const [iterations, setIterations] = useState<ImprovementIteration[]>([]);
  const [systemHealth, setSystemHealth] = useState<SystemHealth | null>(null);
  const [benchmarkResults, setBenchmarkResults] = useState<BenchmarkResult[]>(
    []
  );
  const [configJson, setConfigJson] = useState("");
  const [selectedBenchmark, setSelectedBenchmark] = useState<string>("quick");
  const [isInitialized, setIsInitialized] = useState(false);
  const [backendAvailable, setBackendAvailable] = useState(true);

  // Clear messages after timeout
  useEffect(() => {
    if (error) {
      const timeout = setTimeout(() => setError(null), 5000);
      return () => clearTimeout(timeout);
    }
  }, [error]);

  useEffect(() => {
    if (success) {
      const timeout = setTimeout(() => setSuccess(null), 3000);
      return () => clearTimeout(timeout);
    }
  }, [success]);

  // Initialize the system
  const initializeSystem = useCallback(async () => {
    if (!DEVELOPMENT_MODE_ONLY) {
      setError("Self-improvement system is only available in development mode");
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      const request: InitializeSelfImprovementRequest = {
        config: null, // Use default configuration
        load_existing_archive: true,
      };
      await invoke("initialize_self_improvement", { request });
      setIsInitialized(true);
      setSuccess("Self-improvement system initialized successfully");
      await refreshStatus();
    } catch (err) {
      setError(`Failed to initialize: ${err}`);
    } finally {
      setIsLoading(false);
    }
  }, []);

  // Get current status
  const refreshStatus = useCallback(async () => {
    if (!DEVELOPMENT_MODE_ONLY || !backendAvailable) return;

    try {
      const statusData = await invoke("get_self_improvement_status");
      setStatus(statusData as SelfImprovementStatus);
    } catch (err) {
      // If the command doesn't exist, stop polling to avoid console spam
      if (typeof err === "string" && err.includes("not found")) {
        setBackendAvailable(false);
        return;
      }
      console.error("Failed to get status:", err);
    }
  }, [backendAvailable]);

  // Start improvement cycle
  const startImprovementCycle = useCallback(async () => {
    if (!DEVELOPMENT_MODE_ONLY) {
      setError("Self-improvement system is only available in development mode");
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      await invoke("start_improvement_cycle");
      setSuccess("Improvement cycle started successfully");
      await refreshStatus();
    } catch (err) {
      setError(`Failed to start improvement cycle: ${err}`);
    } finally {
      setIsLoading(false);
    }
  }, [refreshStatus]);

  // Emergency stop
  const emergencyStop = useCallback(async () => {
    if (!DEVELOPMENT_MODE_ONLY) return;

    setIsLoading(true);
    setError(null);

    try {
      await invoke("emergency_stop_improvement");
      setSuccess("Emergency stop completed successfully");
      await refreshStatus();
    } catch (err) {
      setError(`Failed to stop improvement: ${err}`);
    } finally {
      setIsLoading(false);
    }
  }, [refreshStatus]);

  // Analyze system performance
  const analyzeSystem = useCallback(async () => {
    if (!DEVELOPMENT_MODE_ONLY) return;

    setIsLoading(true);
    setError(null);

    try {
      const analysis = await invoke("analyze_system_performance");
      setSuccess("System analysis completed");
      console.log("Analysis results:", analysis);
    } catch (err) {
      setError(`Failed to analyze system: ${err}`);
    } finally {
      setIsLoading(false);
    }
  }, []);

  // Get improvement archive
  const getArchive = useCallback(async () => {
    if (!DEVELOPMENT_MODE_ONLY) return;

    try {
      const archive = await invoke("get_improvement_archive");
      setIterations(archive as ImprovementIteration[]);
    } catch (err) {
      console.error("Failed to get archive:", err);
    }
  }, []);

  // Get system health
  const getSystemHealth = useCallback(async () => {
    if (!DEVELOPMENT_MODE_ONLY) return;

    try {
      const health = await invoke("get_system_health_metrics");
      setSystemHealth(health as SystemHealth);
    } catch (err) {
      console.error("Failed to get system health:", err);
    }
  }, []);

  // Run benchmarks
  const runBenchmark = useCallback(async () => {
    if (!DEVELOPMENT_MODE_ONLY) return;

    setIsLoading(true);
    setError(null);

    try {
      const results = await invoke("run_performance_benchmarks", {
        benchmark_type: selectedBenchmark,
      });

      // Convert the backend results to our BenchmarkResult interface
      const convertedResults = (results as any[]).map((result: any) => ({
        benchmark_type: result.benchmark_type,
        score: result.score,
        target: result.target,
        status: result.status,
        details: result.details,
      }));

      setBenchmarkResults(convertedResults as BenchmarkResult[]);
      setSuccess(
        `Benchmark '${selectedBenchmark}' completed successfully (${convertedResults.length} tests)`
      );
    } catch (err) {
      setError(`Failed to run benchmark: ${err}`);
    } finally {
      setIsLoading(false);
    }
  }, [selectedBenchmark]);

  // Update configuration
  const updateConfiguration = useCallback(async () => {
    if (!DEVELOPMENT_MODE_ONLY) return;

    setIsLoading(true);
    setError(null);

    try {
      // Validate JSON
      JSON.parse(configJson);
      await invoke("update_self_improvement_config", {
        configJson,
      });
      setSuccess("Configuration updated successfully");
    } catch (err) {
      if (err instanceof SyntaxError) {
        setError("Invalid JSON configuration");
      } else {
        setError(`Failed to update configuration: ${err}`);
      }
    } finally {
      setIsLoading(false);
    }
  }, [configJson]);

  // Generate improvement proposal
  const generateProposal = useCallback(async () => {
    if (!DEVELOPMENT_MODE_ONLY) return;

    setIsLoading(true);
    setError(null);

    try {
      const proposal = await invoke("generate_improvement_proposal");
      setSuccess("Improvement proposal generated");
      console.log("Proposal:", proposal);
    } catch (err) {
      setError(`Failed to generate proposal: ${err}`);
    } finally {
      setIsLoading(false);
    }
  }, []);

  // Load data when tab changes
  useEffect(() => {
    if (!DEVELOPMENT_MODE_ONLY) return;

    switch (activeTab) {
      case "overview":
        refreshStatus();
        break;
      case "archive":
        getArchive();
        break;
      case "health":
        getSystemHealth();
        break;
      default:
        break;
    }
  }, [activeTab, refreshStatus, getArchive, getSystemHealth]);

  // Auto-refresh status every 30 seconds when active (only if backend commands exist)
  useEffect(() => {
    if (!DEVELOPMENT_MODE_ONLY || activeTab !== "overview" || !backendAvailable) return;

    const interval = setInterval(refreshStatus, 30000);
    return () => clearInterval(interval);
  }, [activeTab, refreshStatus, backendAvailable]);

  // Development mode warning
  if (!DEVELOPMENT_MODE_ONLY) {
    return (
      <Card className="w-full">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Brain className="h-5 w-5" />
            Self-Improvement System
          </CardTitle>
          <CardDescription>
            Research-backed autonomous code improvement system
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Alert>
            <AlertTriangle className="h-4 w-4" />
            <AlertTitle>Development Mode Only</AlertTitle>
            <AlertDescription>
              The self-improvement system is only available in development mode
              for safety. Please run the application in development mode to
              access these features.
            </AlertDescription>
          </Alert>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className="w-full">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Brain className="h-5 w-5" />
          Self-Improvement System
          <Badge variant={status?.is_active ? "default" : "secondary"}>
            {status?.is_active ? "Active" : "Inactive"}
          </Badge>
        </CardTitle>
        <CardDescription>
          Research-backed autonomous code improvement system (Development Mode)
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Status Messages */}
        {error && (
          <Alert variant="destructive">
            <AlertTriangle className="h-4 w-4" />
            <AlertTitle>Error</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}

        {success && (
          <Alert>
            <CheckCircle className="h-4 w-4" />
            <AlertTitle>Success</AlertTitle>
            <AlertDescription>{success}</AlertDescription>
          </Alert>
        )}

        {/* Quick Actions */}
        <div className="flex flex-wrap gap-2">
          {!isInitialized && (
            <Button
              onClick={initializeSystem}
              disabled={isLoading}
              className="flex items-center gap-2"
            >
              <Zap className="h-4 w-4" />
              Initialize System
            </Button>
          )}

          {isInitialized && (
            <>
              <Button
                onClick={startImprovementCycle}
                disabled={isLoading || status?.is_active}
                className="flex items-center gap-2"
              >
                <Play className="h-4 w-4" />
                Start Cycle
              </Button>

              <Button
                onClick={emergencyStop}
                disabled={isLoading || !status?.is_active}
                variant="destructive"
                className="flex items-center gap-2"
              >
                <Square className="h-4 w-4" />
                Emergency Stop
              </Button>

              <Button
                onClick={refreshStatus}
                disabled={isLoading}
                variant="outline"
                className="flex items-center gap-2"
              >
                <RefreshCw className="h-4 w-4" />
                Refresh
              </Button>
            </>
          )}
        </div>

        <Separator />

        {/* Main Content Tabs */}
        <Tabs value={activeTab} onValueChange={setActiveTab}>
          <TabsList className="grid w-full grid-cols-5">
            <TabsTrigger value="overview">Overview</TabsTrigger>
            <TabsTrigger value="archive">Archive</TabsTrigger>
            <TabsTrigger value="health">Health</TabsTrigger>
            <TabsTrigger value="benchmarks">Benchmarks</TabsTrigger>
            <TabsTrigger value="config">Config</TabsTrigger>
          </TabsList>

          {/* Overview Tab */}
          <TabsContent value="overview" className="space-y-4">
            {status && (
              <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                <Card>
                  <CardHeader className="pb-2">
                    <CardTitle className="text-sm flex items-center gap-2">
                      <TrendingUp className="h-4 w-4" />
                      Cycles
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <div className="text-2xl font-bold">
                      {status.cycles_completed}
                    </div>
                    <p className="text-xs text-muted-foreground">Completed</p>
                  </CardContent>
                </Card>

                <Card>
                  <CardHeader className="pb-2">
                    <CardTitle className="text-sm flex items-center gap-2">
                      <Target className="h-4 w-4" />
                      Success Rate
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <div className="text-2xl font-bold">
                      {status.success_rate}%
                    </div>
                    <Progress value={status.success_rate} className="mt-2" />
                  </CardContent>
                </Card>

                <Card>
                  <CardHeader className="pb-2">
                    <CardTitle className="text-sm flex items-center gap-2">
                      <Zap className="h-4 w-4" />
                      Performance
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <div className="text-2xl font-bold text-green-600">
                      +{status.performance_gain}%
                    </div>
                    <p className="text-xs text-muted-foreground">Improvement</p>
                  </CardContent>
                </Card>

                <Card>
                  <CardHeader className="pb-2">
                    <CardTitle className="text-sm flex items-center gap-2">
                      <Database className="h-4 w-4" />
                      Memory
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <div className="text-2xl font-bold">
                      {status.memory_usage}MB
                    </div>
                    <p className="text-xs text-muted-foreground">Usage</p>
                  </CardContent>
                </Card>
              </div>
            )}

            {status?.active_agents && status.active_agents.length > 0 && (
              <Card>
                <CardHeader>
                  <CardTitle className="text-sm">Active Agents</CardTitle>
                </CardHeader>
                <CardContent>
                  <div className="flex flex-wrap gap-2">
                    {status.active_agents.map((agent, index) => (
                      <Badge key={index} variant="outline">
                        {agent}
                      </Badge>
                    ))}
                  </div>
                </CardContent>
              </Card>
            )}

            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <Button
                onClick={analyzeSystem}
                disabled={isLoading}
                variant="outline"
                className="flex items-center gap-2"
              >
                <BarChart3 className="h-4 w-4" />
                Analyze System
              </Button>

              <Button
                onClick={generateProposal}
                disabled={isLoading}
                variant="outline"
                className="flex items-center gap-2"
              >
                <Brain className="h-4 w-4" />
                Generate Proposal
              </Button>
            </div>
          </TabsContent>

          {/* Archive Tab */}
          <TabsContent value="archive" className="space-y-4">
            <div className="flex items-center justify-between">
              <h3 className="text-lg font-semibold flex items-center gap-2">
                <Archive className="h-5 w-5" />
                Improvement History
              </h3>
              <Button
                onClick={getArchive}
                disabled={isLoading}
                variant="outline"
                size="sm"
              >
                <RefreshCw className="h-4 w-4" />
              </Button>
            </div>

            <div className="space-y-2">
              {iterations.map((iteration) => (
                <Card key={iteration.id}>
                  <CardContent className="pt-4">
                    <div className="flex items-center justify-between">
                      <div>
                        <div className="font-medium">
                          Iteration {iteration.id}
                        </div>
                        <div className="text-sm text-muted-foreground">
                          {iteration.timestamp} • {iteration.duration_minutes}
                          min
                        </div>
                      </div>
                      <div className="text-right">
                        <div className="text-lg font-bold text-green-600">
                          +{iteration.performance_gain}%
                        </div>
                        <Badge
                          variant={
                            iteration.status === "completed"
                              ? "default"
                              : "secondary"
                          }
                        >
                          {iteration.status}
                        </Badge>
                      </div>
                    </div>
                    <div className="mt-2 text-sm text-muted-foreground">
                      {iteration.improvements_applied} improvements •{" "}
                      {iteration.safety_checks_passed} safety checks passed
                    </div>
                  </CardContent>
                </Card>
              ))}
            </div>
          </TabsContent>

          {/* Health Tab */}
          <TabsContent value="health" className="space-y-4">
            <div className="flex items-center justify-between">
              <h3 className="text-lg font-semibold flex items-center gap-2">
                <Activity className="h-5 w-5" />
                System Health
              </h3>
              <Button
                onClick={getSystemHealth}
                disabled={isLoading}
                variant="outline"
                size="sm"
              >
                <RefreshCw className="h-4 w-4" />
              </Button>
            </div>

            {systemHealth && (
              <div className="space-y-4">
                <Card>
                  <CardHeader>
                    <CardTitle className="flex items-center gap-2">
                      <Gauge className="h-5 w-5" />
                      Overall Health Score
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <div className="text-3xl font-bold text-green-600">
                      {systemHealth.overall_score}/100
                    </div>
                    <Progress
                      value={systemHealth.overall_score}
                      className="mt-2"
                    />
                  </CardContent>
                </Card>

                <Card>
                  <CardHeader>
                    <CardTitle>Component Status</CardTitle>
                  </CardHeader>
                  <CardContent>
                    <div className="space-y-2">
                      {systemHealth.components &&
                      Object.keys(systemHealth.components).length > 0 ? (
                        Object.entries(systemHealth.components).map(
                          ([component, status]) => (
                            <div
                              key={component}
                              className="flex items-center justify-between"
                            >
                              <span>{component}</span>
                              <Badge
                                variant={
                                  status === "healthy"
                                    ? "default"
                                    : "destructive"
                                }
                              >
                                {status}
                              </Badge>
                            </div>
                          )
                        )
                      ) : (
                        <div className="text-sm text-muted-foreground">
                          No component data available
                        </div>
                      )}
                    </div>
                  </CardContent>
                </Card>

                {systemHealth.recommendations &&
                  systemHealth.recommendations.length > 0 && (
                    <Card>
                      <CardHeader>
                        <CardTitle>Recommendations</CardTitle>
                      </CardHeader>
                      <CardContent>
                        <ul className="space-y-1">
                          {systemHealth.recommendations.map((rec, index) => (
                            <li
                              key={index}
                              className="text-sm flex items-start gap-2"
                            >
                              <Info className="h-4 w-4 mt-0.5 text-blue-500" />
                              {rec}
                            </li>
                          ))}
                        </ul>
                      </CardContent>
                    </Card>
                  )}
              </div>
            )}
          </TabsContent>

          {/* Benchmarks Tab */}
          <TabsContent value="benchmarks" className="space-y-4">
            <div className="flex items-center gap-4">
              <Select
                value={selectedBenchmark}
                onValueChange={setSelectedBenchmark}
              >
                <SelectTrigger className="w-48">
                  <SelectValue placeholder="Select benchmark" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All Benchmarks</SelectItem>
                  <SelectItem value="quick">Quick Suite (4s)</SelectItem>
                  <SelectItem value="core">Core Suite (6s)</SelectItem>
                  <SelectItem value="advanced">Advanced Suite (10s)</SelectItem>
                  <SelectItem value="accuracy">Accuracy</SelectItem>
                  <SelectItem value="performance">Performance</SelectItem>
                  <SelectItem value="reliability">Reliability</SelectItem>
                  <SelectItem value="cost">Cost Efficiency</SelectItem>
                  <SelectItem value="innovation">Innovation</SelectItem>
                </SelectContent>
              </Select>

              <Button
                onClick={runBenchmark}
                disabled={isLoading}
                className="flex items-center gap-2"
              >
                <Play className="h-4 w-4" />
                Run Benchmark
              </Button>
            </div>

            {benchmarkResults.length > 0 && (
              <div className="space-y-2">
                {benchmarkResults.map((result, index) => (
                  <Card key={index}>
                    <CardContent className="pt-4">
                      <div className="flex items-center justify-between">
                        <div>
                          <div className="font-medium">
                            {result.benchmark_type}
                          </div>
                          <div className="text-sm text-muted-foreground">
                            Target: {result.target}
                          </div>
                        </div>
                        <div className="text-right">
                          <div className="text-lg font-bold">
                            {result.score}
                          </div>
                          <Badge
                            variant={
                              result.status === "passed"
                                ? "default"
                                : "destructive"
                            }
                          >
                            {result.status}
                          </Badge>
                        </div>
                      </div>
                      {result.details && (
                        <div className="mt-2 text-sm text-muted-foreground">
                          {result.details}
                        </div>
                      )}
                    </CardContent>
                  </Card>
                ))}
              </div>
            )}
          </TabsContent>

          {/* Configuration Tab */}
          <TabsContent value="config" className="space-y-4">
            <div>
              <h3 className="text-lg font-semibold flex items-center gap-2 mb-4">
                <Settings className="h-5 w-5" />
                Configuration
              </h3>

              <div className="space-y-4">
                <div>
                  <label className="text-sm font-medium">
                    Configuration JSON
                  </label>
                  <Textarea
                    value={configJson}
                    onChange={(e) => setConfigJson(e.target.value)}
                    placeholder={
                      '{\n  "max_iterations": 10,\n  "safety_threshold": 0.9,\n  "performance_target": 0.15\n}'
                    }
                    rows={10}
                    className="mt-1"
                  />
                </div>

                <Button
                  onClick={updateConfiguration}
                  disabled={isLoading || !configJson.trim()}
                  className="flex items-center gap-2"
                >
                  <Settings className="h-4 w-4" />
                  Update Configuration
                </Button>
              </div>
            </div>
          </TabsContent>
        </Tabs>

        {/* Footer Info */}
        <Separator />
        <div className="text-xs text-muted-foreground flex items-center gap-4">
          <div className="flex items-center gap-1">
            <Shield className="h-3 w-3" />
            Development Mode Only
          </div>
          <div className="flex items-center gap-1">
            <Brain className="h-3 w-3" />
            Research-Backed (17-53% gains)
          </div>
          {status?.last_update && (
            <div className="flex items-center gap-1">
              <Clock className="h-3 w-3" />
              Last Update: {status.last_update}
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
};

export default SelfImprovementPanel;
