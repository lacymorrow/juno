import React, { useState, useEffect } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from './ui/card';
import { Button } from './ui/button';
import { Switch } from './ui/switch';
import { Badge } from './ui/badge';
import { Alert, AlertDescription } from './ui/alert';
import { Tabs, TabsContent, TabsList, TabsTrigger } from './ui/tabs';
import { Input } from './ui/input';
import { Label } from './ui/label';
import { Textarea } from './ui/textarea';
import { 
  Shield, 
  ShieldCheck, 
  ShieldAlert, 
  Settings, 
  AlertTriangle,
  Save,
  RotateCcw,
  Plus,
  Trash2,
  Info,
  Lock,
  Unlock,
  Eye,
  EyeOff
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

interface SecurityConfig {
  enabled: boolean;
  development_mode: boolean;
  auto_block_critical: boolean;
  require_approval_for_high_risk: boolean;
  require_approval_for_medium_risk: boolean;
  log_all_commands: boolean;
  rate_limiting: {
    enabled: boolean;
    max_commands_per_minute: number;
    max_dangerous_commands_per_hour: number;
    violation_cooldown_minutes: number;
  };
  file_monitoring: {
    enabled: boolean;
    monitor_system_files: boolean;
    alert_on_sensitive_access: boolean;
  };
  approval_settings: {
    default_timeout_seconds: number;
    remember_decisions: boolean;
    require_reason_for_dangerous: boolean;
  };
  custom_patterns: {
    blocked_patterns: string[];
    allowed_patterns: string[];
    monitored_directories: string[];
  };
}

interface SecurityStats {
  total_commands_validated: number;
  commands_blocked: number;
  commands_allowed: number;
  uptime_hours: number;
  last_violation: string | null;
}

export function SecurityConfigurationPanel() {
  const [config, setConfig] = useState<SecurityConfig | null>(null);
  const [stats, setStats] = useState<SecurityStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [newPattern, setNewPattern] = useState('');
  const [newDirectory, setNewDirectory] = useState('');
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [testCommand, setTestCommand] = useState('');
  const [testResult, setTestResult] = useState<string | null>(null);

  // Load configuration and stats
  const loadData = async () => {
    try {
      setLoading(true);
      const [configResult, statsResult] = await Promise.all([
        invoke<SecurityConfig>('get_security_config'),
        invoke<SecurityStats>('get_security_stats')
      ]);
      setConfig(configResult);
      setStats(statsResult);
    } catch (error) {
      console.error('Failed to load security data:', error);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  // Save configuration
  const saveConfiguration = async () => {
    if (!config) return;
    
    try {
      setSaving(true);
      await invoke('update_security_config', { config });
      await loadData(); // Refresh data
    } catch (error) {
      console.error('Failed to save security configuration:', error);
    } finally {
      setSaving(false);
    }
  };

  // Reset to defaults
  const resetToDefaults = async () => {
    try {
      setSaving(true);
      await invoke('reset_security_config');
      await loadData();
    } catch (error) {
      console.error('Failed to reset security configuration:', error);
    } finally {
      setSaving(false);
    }
  };

  // Test command against security
  const testCommandSecurity = async () => {
    if (!testCommand.trim()) return;
    
    try {
      const result = await invoke<{allowed: boolean, reason: string, risk_level: string}>(
        'test_command_security', 
        { command: testCommand }
      );
      setTestResult(`${result.allowed ? '✅ ALLOWED' : '🚫 BLOCKED'} (${result.risk_level}): ${result.reason}`);
    } catch (error) {
      setTestResult(`❌ Error testing command: ${error}`);
    }
  };

  // Add custom pattern
  const addBlockedPattern = () => {
    if (!newPattern.trim() || !config) return;
    
    setConfig({
      ...config,
      custom_patterns: {
        ...config.custom_patterns,
        blocked_patterns: [...config.custom_patterns.blocked_patterns, newPattern.trim()]
      }
    });
    setNewPattern('');
  };

  // Remove pattern
  const removeBlockedPattern = (index: number) => {
    if (!config) return;
    
    const patterns = [...config.custom_patterns.blocked_patterns];
    patterns.splice(index, 1);
    setConfig({
      ...config,
      custom_patterns: {
        ...config.custom_patterns,
        blocked_patterns: patterns
      }
    });
  };

  // Add monitored directory
  const addMonitoredDirectory = () => {
    if (!newDirectory.trim() || !config) return;
    
    setConfig({
      ...config,
      custom_patterns: {
        ...config.custom_patterns,
        monitored_directories: [...config.custom_patterns.monitored_directories, newDirectory.trim()]
      }
    });
    setNewDirectory('');
  };

  // Remove directory
  const removeMonitoredDirectory = (index: number) => {
    if (!config) return;
    
    const directories = [...config.custom_patterns.monitored_directories];
    directories.splice(index, 1);
    setConfig({
      ...config,
      custom_patterns: {
        ...config.custom_patterns,
        monitored_directories: directories
      }
    });
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center p-8">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
        <span className="ml-2">Loading security configuration...</span>
      </div>
    );
  }

  if (!config) {
    return (
      <Alert className="border-red-200 bg-red-50">
        <AlertTriangle className="w-4 h-4 text-red-600" />
        <AlertDescription className="text-red-800">
          Failed to load security configuration. Please try refreshing the page.
        </AlertDescription>
      </Alert>
    );
  }

  const protectionRate = stats ? ((stats.commands_blocked / Math.max(stats.total_commands_validated, 1)) * 100).toFixed(1) : '0.0';

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <Shield className="w-6 h-6 text-blue-600" />
          <div>
            <h2 className="text-2xl font-bold">Security Configuration</h2>
            <p className="text-gray-600">Configure security policies and monitoring settings</p>
          </div>
        </div>
        
        <div className="flex items-center gap-2">
          <Button onClick={resetToDefaults} variant="outline" size="sm" disabled={saving}>
            <RotateCcw className="w-4 h-4 mr-2" />
            Reset to Defaults
          </Button>
          <Button onClick={saveConfiguration} disabled={saving} size="sm">
            <Save className="w-4 h-4 mr-2" />
            {saving ? 'Saving...' : 'Save Changes'}
          </Button>
        </div>
      </div>

      {/* Security Status Overview */}
      {stats && (
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <ShieldCheck className="w-5 h-5 text-green-600" />
              Security Status Overview
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
              <div className="text-center">
                <div className="text-2xl font-bold text-blue-600">{stats.total_commands_validated}</div>
                <div className="text-sm text-gray-600">Commands Validated</div>
              </div>
              <div className="text-center">
                <div className="text-2xl font-bold text-red-600">{stats.commands_blocked}</div>
                <div className="text-sm text-gray-600">Commands Blocked</div>
              </div>
              <div className="text-center">
                <div className="text-2xl font-bold text-green-600">{stats.commands_allowed}</div>
                <div className="text-sm text-gray-600">Commands Allowed</div>
              </div>
              <div className="text-center">
                <div className="text-2xl font-bold text-purple-600">{protectionRate}%</div>
                <div className="text-sm text-gray-600">Protection Rate</div>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Main Configuration Tabs */}
      <Tabs defaultValue="general" className="w-full">
        <TabsList className="grid w-full grid-cols-4">
          <TabsTrigger value="general">General</TabsTrigger>
          <TabsTrigger value="policies">Policies</TabsTrigger>
          <TabsTrigger value="monitoring">Monitoring</TabsTrigger>
          <TabsTrigger value="advanced">Advanced</TabsTrigger>
        </TabsList>

        {/* General Settings */}
        <TabsContent value="general" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Core Security Settings</CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="flex items-center justify-between">
                <div className="space-y-1">
                  <Label className="text-base font-medium">Enable Security System</Label>
                  <p className="text-sm text-gray-600">
                    Master switch for all security features. When disabled, all commands execute without validation.
                  </p>
                </div>
                <Switch
                  checked={config.enabled}
                  onCheckedChange={(checked) => setConfig({...config, enabled: checked})}
                />
              </div>

              <div className="flex items-center justify-between">
                <div className="space-y-1">
                  <Label className="text-base font-medium">Development Mode</Label>
                  <p className="text-sm text-gray-600">
                    Less restrictive policies for development. Enables additional debugging and self-awareness tools.
                  </p>
                </div>
                <Switch
                  checked={config.development_mode}
                  onCheckedChange={(checked) => setConfig({...config, development_mode: checked})}
                />
              </div>

              <div className="flex items-center justify-between">
                <div className="space-y-1">
                  <Label className="text-base font-medium">Auto-block Critical Commands</Label>
                  <p className="text-sm text-gray-600">
                    Automatically block dangerous commands like "rm -rf /" without user approval.
                  </p>
                </div>
                <Switch
                  checked={config.auto_block_critical}
                  onCheckedChange={(checked) => setConfig({...config, auto_block_critical: checked})}
                />
              </div>

              <div className="flex items-center justify-between">
                <div className="space-y-1">
                  <Label className="text-base font-medium">Log All Commands</Label>
                  <p className="text-sm text-gray-600">
                    Record all command executions for audit and debugging purposes.
                  </p>
                </div>
                <Switch
                  checked={config.log_all_commands}
                  onCheckedChange={(checked) => setConfig({...config, log_all_commands: checked})}
                />
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        {/* Policy Settings */}
        <TabsContent value="policies" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Approval Policies</CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="flex items-center justify-between">
                <div className="space-y-1">
                  <Label className="text-base font-medium">Require Approval for High Risk Commands</Label>
                  <p className="text-sm text-gray-600">
                    Commands like package management and system configuration changes.
                  </p>
                </div>
                <Switch
                  checked={config.require_approval_for_high_risk}
                  onCheckedChange={(checked) => setConfig({...config, require_approval_for_high_risk: checked})}
                />
              </div>

              <div className="flex items-center justify-between">
                <div className="space-y-1">
                  <Label className="text-base font-medium">Require Approval for Medium Risk Commands</Label>
                  <p className="text-sm text-gray-600">
                    File operations, network commands, and process management.
                  </p>
                </div>
                <Switch
                  checked={config.require_approval_for_medium_risk}
                  onCheckedChange={(checked) => setConfig({...config, require_approval_for_medium_risk: checked})}
                />
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="space-y-2">
                  <Label>Approval Timeout (seconds)</Label>
                  <Input
                    type="number"
                    value={config.approval_settings.default_timeout_seconds}
                    onChange={(e) => setConfig({
                      ...config,
                      approval_settings: {
                        ...config.approval_settings,
                        default_timeout_seconds: parseInt(e.target.value) || 60
                      }
                    })}
                    min={10}
                    max={300}
                  />
                  <p className="text-xs text-gray-600">Time before auto-denying approval requests</p>
                </div>

                <div className="space-y-3">
                  <div className="flex items-center space-x-2">
                    <Switch
                      id="remember-decisions"
                      checked={config.approval_settings.remember_decisions}
                      onCheckedChange={(checked) => setConfig({
                        ...config,
                        approval_settings: {
                          ...config.approval_settings,
                          remember_decisions: checked
                        }
                      })}
                    />
                    <Label htmlFor="remember-decisions">Remember approval decisions</Label>
                  </div>
                  
                  <div className="flex items-center space-x-2">
                    <Switch
                      id="require-reason"
                      checked={config.approval_settings.require_reason_for_dangerous}
                      onCheckedChange={(checked) => setConfig({
                        ...config,
                        approval_settings: {
                          ...config.approval_settings,
                          require_reason_for_dangerous: checked
                        }
                      })}
                    />
                    <Label htmlFor="require-reason">Require reason for dangerous commands</Label>
                  </div>
                </div>
              </div>
            </CardContent>
          </Card>

          {/* Command Testing */}
          <Card>
            <CardHeader>
              <CardTitle>Command Security Testing</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex gap-2">
                <Input
                  placeholder="Enter a command to test (e.g., 'rm file.txt' or 'sudo apt install')"
                  value={testCommand}
                  onChange={(e) => setTestCommand(e.target.value)}
                  onKeyPress={(e) => e.key === 'Enter' && testCommandSecurity()}
                />
                <Button onClick={testCommandSecurity} disabled={!testCommand.trim()}>
                  Test
                </Button>
              </div>
              
              {testResult && (
                <Alert className={testResult.includes('ALLOWED') ? 'border-green-200 bg-green-50' : 'border-red-200 bg-red-50'}>
                  <AlertDescription className={testResult.includes('ALLOWED') ? 'text-green-800' : 'text-red-800'}>
                    {testResult}
                  </AlertDescription>
                </Alert>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        {/* Monitoring Settings */}
        <TabsContent value="monitoring" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Rate Limiting</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex items-center justify-between">
                <Label className="text-base font-medium">Enable Rate Limiting</Label>
                <Switch
                  checked={config.rate_limiting.enabled}
                  onCheckedChange={(checked) => setConfig({
                    ...config,
                    rate_limiting: { ...config.rate_limiting, enabled: checked }
                  })}
                />
              </div>

              <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div className="space-y-2">
                  <Label>Max Commands/Minute</Label>
                  <Input
                    type="number"
                    value={config.rate_limiting.max_commands_per_minute}
                    onChange={(e) => setConfig({
                      ...config,
                      rate_limiting: {
                        ...config.rate_limiting,
                        max_commands_per_minute: parseInt(e.target.value) || 60
                      }
                    })}
                    min={1}
                    max={1000}
                  />
                </div>

                <div className="space-y-2">
                  <Label>Max Dangerous/Hour</Label>
                  <Input
                    type="number"
                    value={config.rate_limiting.max_dangerous_commands_per_hour}
                    onChange={(e) => setConfig({
                      ...config,
                      rate_limiting: {
                        ...config.rate_limiting,
                        max_dangerous_commands_per_hour: parseInt(e.target.value) || 10
                      }
                    })}
                    min={1}
                    max={100}
                  />
                </div>

                <div className="space-y-2">
                  <Label>Violation Cooldown (min)</Label>
                  <Input
                    type="number"
                    value={config.rate_limiting.violation_cooldown_minutes}
                    onChange={(e) => setConfig({
                      ...config,
                      rate_limiting: {
                        ...config.rate_limiting,
                        violation_cooldown_minutes: parseInt(e.target.value) || 5
                      }
                    })}
                    min={1}
                    max={60}
                  />
                </div>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>File System Monitoring</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex items-center justify-between">
                <Label className="text-base font-medium">Enable File Monitoring</Label>
                <Switch
                  checked={config.file_monitoring.enabled}
                  onCheckedChange={(checked) => setConfig({
                    ...config,
                    file_monitoring: { ...config.file_monitoring, enabled: checked }
                  })}
                />
              </div>

              <div className="flex items-center justify-between">
                <Label className="text-base font-medium">Monitor System Files</Label>
                <Switch
                  checked={config.file_monitoring.monitor_system_files}
                  onCheckedChange={(checked) => setConfig({
                    ...config,
                    file_monitoring: { ...config.file_monitoring, monitor_system_files: checked }
                  })}
                />
              </div>

              <div className="flex items-center justify-between">
                <Label className="text-base font-medium">Alert on Sensitive Access</Label>
                <Switch
                  checked={config.file_monitoring.alert_on_sensitive_access}
                  onCheckedChange={(checked) => setConfig({
                    ...config,
                    file_monitoring: { ...config.file_monitoring, alert_on_sensitive_access: checked }
                  })}
                />
              </div>

              {/* Monitored Directories */}
              <div className="space-y-3">
                <Label className="text-base font-medium">Monitored Directories</Label>
                <div className="flex gap-2">
                  <Input
                    placeholder="Enter directory path to monitor"
                    value={newDirectory}
                    onChange={(e) => setNewDirectory(e.target.value)}
                    onKeyPress={(e) => e.key === 'Enter' && addMonitoredDirectory()}
                  />
                  <Button onClick={addMonitoredDirectory} size="sm" disabled={!newDirectory.trim()}>
                    <Plus className="w-4 h-4" />
                  </Button>
                </div>
                
                <div className="space-y-2 max-h-40 overflow-y-auto">
                  {config.custom_patterns.monitored_directories.map((dir, index) => (
                    <div key={index} className="flex items-center justify-between p-2 bg-gray-50 rounded">
                      <code className="text-sm">{dir}</code>
                      <Button
                        onClick={() => removeMonitoredDirectory(index)}
                        variant="ghost"
                        size="sm"
                      >
                        <Trash2 className="w-4 h-4 text-red-600" />
                      </Button>
                    </div>
                  ))}
                </div>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        {/* Advanced Settings */}
        <TabsContent value="advanced" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Settings className="w-5 h-5" />
                Custom Security Patterns
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <Alert className="border-orange-200 bg-orange-50">
                <AlertTriangle className="w-4 h-4 text-orange-600" />
                <AlertDescription className="text-orange-800">
                  <strong>Advanced users only:</strong> Incorrect patterns may block legitimate commands or allow dangerous ones.
                </AlertDescription>
              </Alert>

              <div className="space-y-3">
                <Label className="text-base font-medium">Custom Blocked Patterns (Regex)</Label>
                <div className="flex gap-2">
                  <Input
                    placeholder="Enter regex pattern to block (e.g., 'rm.*-rf.*')"
                    value={newPattern}
                    onChange={(e) => setNewPattern(e.target.value)}
                    onKeyPress={(e) => e.key === 'Enter' && addBlockedPattern()}
                  />
                  <Button onClick={addBlockedPattern} size="sm" disabled={!newPattern.trim()}>
                    <Plus className="w-4 h-4" />
                  </Button>
                </div>
                
                <div className="space-y-2 max-h-40 overflow-y-auto">
                  {config.custom_patterns.blocked_patterns.map((pattern, index) => (
                    <div key={index} className="flex items-center justify-between p-2 bg-red-50 rounded border border-red-200">
                      <code className="text-sm text-red-800">{pattern}</code>
                      <Button
                        onClick={() => removeBlockedPattern(index)}
                        variant="ghost"
                        size="sm"
                      >
                        <Trash2 className="w-4 h-4 text-red-600" />
                      </Button>
                    </div>
                  ))}
                </div>
              </div>

              <div className="flex items-center gap-2 mt-6">
                <Button
                  onClick={() => setShowAdvanced(!showAdvanced)}
                  variant="outline"
                  size="sm"
                >
                  {showAdvanced ? <EyeOff className="w-4 h-4 mr-2" /> : <Eye className="w-4 h-4 mr-2" />}
                  {showAdvanced ? 'Hide' : 'Show'} Raw Configuration
                </Button>
              </div>

              {showAdvanced && (
                <div className="space-y-2">
                  <Label>Raw JSON Configuration (Read-only)</Label>
                  <Textarea
                    value={JSON.stringify(config, null, 2)}
                    readOnly
                    rows={15}
                    className="font-mono text-xs"
                  />
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}