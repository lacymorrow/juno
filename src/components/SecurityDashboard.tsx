import React, { useState, useEffect } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from './ui/card';
import { Badge } from './ui/badge';
import { Button } from './ui/button';
import { Alert, AlertDescription } from './ui/alert';
import { Tabs, TabsContent, TabsList, TabsTrigger } from './ui/tabs';
import { 
  Shield, 
  ShieldCheck, 
  ShieldAlert, 
  Activity, 
  Clock, 
  Terminal, 
  FileX, 
  AlertTriangle,
  CheckCircle,
  XCircle,
  TrendingUp,
  Eye,
  RefreshCw
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

interface SecurityStatus {
  security_enabled: boolean;
  total_commands_validated: number;
  commands_blocked: number;
  commands_allowed: number;
  active_monitors: number;
  pending_approvals: number;
}

interface CommandHistoryEntry {
  command: string;
  tool_name: string;
  timestamp: number;
  risk_level: string;
  allowed: boolean;
  execution_time_ms: number;
  exit_code?: number;
}

export function SecurityDashboard() {
  const [securityStatus, setSecurityStatus] = useState<SecurityStatus | null>(null);
  const [commandHistory, setCommandHistory] = useState<CommandHistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadSecurityData = async () => {
    try {
      setLoading(true);
      setError(null);

      const [statusResult, historyResult] = await Promise.all([
        invoke<SecurityStatus>('get_security_status'),
        invoke<CommandHistoryEntry[]>('get_command_history', { limit: 20 })
      ]);

      setSecurityStatus(statusResult);
      setCommandHistory(historyResult);
    } catch (err) {
      setError(`Failed to load security data: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadSecurityData();
    
    // Refresh data every 5 seconds
    const interval = setInterval(loadSecurityData, 5000);
    return () => clearInterval(interval);
  }, []);

  const testDangerousCommands = async () => {
    try {
      await invoke('test_dangerous_commands');
      await loadSecurityData(); // Refresh data after test
    } catch (err) {
      console.error('Test failed:', err);
    }
  };

  const testSafeCommands = async () => {
    try {
      await invoke('test_safe_commands');
      await loadSecurityData(); // Refresh data after test
    } catch (err) {
      console.error('Test failed:', err);
    }
  };

  const getRiskBadge = (riskLevel: string) => {
    switch (riskLevel) {
      case 'Critical':
        return <Badge className="bg-red-100 text-red-800">Critical</Badge>;
      case 'High':
        return <Badge className="bg-orange-100 text-orange-800">High</Badge>;
      case 'Medium':
        return <Badge className="bg-yellow-100 text-yellow-800">Medium</Badge>;
      default:
        return <Badge className="bg-blue-100 text-blue-800">Low</Badge>;
    }
  };

  const formatTimestamp = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleTimeString();
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center p-8">
        <RefreshCw className="w-6 h-6 animate-spin mr-2" />
        Loading security data...
      </div>
    );
  }

  if (error) {
    return (
      <Alert className="border-red-200 bg-red-50">
        <AlertTriangle className="w-4 h-4 text-red-600" />
        <AlertDescription className="text-red-800">{error}</AlertDescription>
      </Alert>
    );
  }

  const securityEnabled = securityStatus?.security_enabled ?? false;
  const totalCommands = securityStatus?.total_commands_validated ?? 0;
  const blockedCommands = securityStatus?.commands_blocked ?? 0;
  const allowedCommands = securityStatus?.commands_allowed ?? 0;
  const activeMonitors = securityStatus?.active_monitors ?? 0;
  const pendingApprovals = securityStatus?.pending_approvals ?? 0;

  const blockRate = totalCommands > 0 ? (blockedCommands / totalCommands * 100).toFixed(1) : '0.0';
  const successRate = totalCommands > 0 ? (allowedCommands / totalCommands * 100).toFixed(1) : '0.0';

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Shield className="w-6 h-6" />
          <h2 className="text-2xl font-bold">Security Dashboard</h2>
          {securityEnabled ? (
            <Badge className="bg-green-100 text-green-800 ml-2">
              <ShieldCheck className="w-3 h-3 mr-1" />
              Active
            </Badge>
          ) : (
            <Badge className="bg-red-100 text-red-800 ml-2">
              <ShieldAlert className="w-3 h-3 mr-1" />
              Disabled
            </Badge>
          )}
        </div>
        
        <Button onClick={loadSecurityData} variant="outline" size="sm">
          <RefreshCw className="w-4 h-4 mr-2" />
          Refresh
        </Button>
      </div>

      {/* Status Overview */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-gray-600">Total Commands</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex items-center gap-2">
              <Terminal className="w-4 h-4 text-blue-600" />
              <span className="text-2xl font-bold">{totalCommands}</span>
            </div>
            <p className="text-xs text-gray-500 mt-1">Commands validated</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-gray-600">Commands Blocked</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex items-center gap-2">
              <XCircle className="w-4 h-4 text-red-600" />
              <span className="text-2xl font-bold text-red-600">{blockedCommands}</span>
            </div>
            <p className="text-xs text-gray-500 mt-1">{blockRate}% block rate</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-gray-600">Commands Allowed</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex items-center gap-2">
              <CheckCircle className="w-4 h-4 text-green-600" />
              <span className="text-2xl font-bold text-green-600">{allowedCommands}</span>
            </div>
            <p className="text-xs text-gray-500 mt-1">{successRate}% success rate</p>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium text-gray-600">Active Monitors</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex items-center gap-2">
              <Eye className="w-4 h-4 text-purple-600" />
              <span className="text-2xl font-bold">{activeMonitors}</span>
            </div>
            <p className="text-xs text-gray-500 mt-1">
              {pendingApprovals} pending approval{pendingApprovals !== 1 ? 's' : ''}
            </p>
          </CardContent>
        </Card>
      </div>

      {/* Main Content Tabs */}
      <Tabs defaultValue="history" className="w-full">
        <TabsList className="grid w-full grid-cols-3">
          <TabsTrigger value="history">Command History</TabsTrigger>
          <TabsTrigger value="metrics">Security Metrics</TabsTrigger>
          <TabsTrigger value="testing">Security Testing</TabsTrigger>
        </TabsList>

        <TabsContent value="history" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Activity className="w-5 h-5" />
                Recent Command History
              </CardTitle>
            </CardHeader>
            <CardContent>
              {commandHistory.length === 0 ? (
                <p className="text-gray-500 text-center py-4">No command history available</p>
              ) : (
                <div className="space-y-2 max-h-96 overflow-y-auto">
                  {commandHistory.map((entry, index) => (
                    <div
                      key={index}
                      className={`p-3 rounded border ${
                        entry.allowed ? 'bg-green-50 border-green-200' : 'bg-red-50 border-red-200'
                      }`}
                    >
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          {entry.allowed ? (
                            <CheckCircle className="w-4 h-4 text-green-600" />
                          ) : (
                            <XCircle className="w-4 h-4 text-red-600" />
                          )}
                          <code className="text-sm font-mono bg-white px-2 py-1 rounded">
                            {entry.command.length > 50 
                              ? `${entry.command.substring(0, 50)}...` 
                              : entry.command}
                          </code>
                          {getRiskBadge(entry.risk_level)}
                        </div>
                        
                        <div className="flex items-center gap-2 text-xs text-gray-500">
                          <span>{entry.tool_name}</span>
                          <Clock className="w-3 h-3" />
                          <span>{formatTimestamp(entry.timestamp)}</span>
                          {entry.execution_time_ms && (
                            <span>({entry.execution_time_ms}ms)</span>
                          )}
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="metrics" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <TrendingUp className="w-5 h-5" />
                Security Metrics
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="space-y-2">
                  <h4 className="font-medium">Protection Effectiveness</h4>
                  <div className="space-y-1">
                    <div className="flex justify-between text-sm">
                      <span>Block Rate:</span>
                      <span className="font-medium text-red-600">{blockRate}%</span>
                    </div>
                    <div className="flex justify-between text-sm">
                      <span>Success Rate:</span>
                      <span className="font-medium text-green-600">{successRate}%</span>
                    </div>
                  </div>
                </div>
                
                <div className="space-y-2">
                  <h4 className="font-medium">System Status</h4>
                  <div className="space-y-1">
                    <div className="flex justify-between text-sm">
                      <span>Security Status:</span>
                      <span className={`font-medium ${securityEnabled ? 'text-green-600' : 'text-red-600'}`}>
                        {securityEnabled ? 'Active' : 'Disabled'}
                      </span>
                    </div>
                    <div className="flex justify-between text-sm">
                      <span>Active Monitors:</span>
                      <span className="font-medium">{activeMonitors}</span>
                    </div>
                  </div>
                </div>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="testing" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <FileX className="w-5 h-5" />
                Security Testing
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <p className="text-sm text-gray-600">
                Test the security system with predefined dangerous and safe commands
              </p>
              
              <div className="flex gap-2">
                <Button 
                  onClick={testDangerousCommands}
                  variant="outline"
                  className="border-red-200 text-red-700 hover:bg-red-50"
                >
                  <ShieldAlert className="w-4 h-4 mr-2" />
                  Test Dangerous Commands
                </Button>
                
                <Button 
                  onClick={testSafeCommands}
                  variant="outline"
                  className="border-green-200 text-green-700 hover:bg-green-50"
                >
                  <ShieldCheck className="w-4 h-4 mr-2" />
                  Test Safe Commands
                </Button>
              </div>
              
              <Alert className="border-blue-200 bg-blue-50">
                <AlertTriangle className="w-4 h-4 text-blue-600" />
                <AlertDescription className="text-blue-800">
                  These tests will verify that the security system correctly blocks dangerous commands 
                  and allows safe ones. Results will appear in the command history above.
                </AlertDescription>
              </Alert>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}