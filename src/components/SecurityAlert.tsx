import React from 'react';
import { Alert, AlertDescription } from './ui/alert';
import { Badge } from './ui/badge';
import { Button } from './ui/button';
import { 
  Shield, 
  ShieldAlert, 
  ShieldCheck, 
  AlertTriangle, 
  CheckCircle, 
  XCircle,
  Clock,
  X
} from 'lucide-react';

export interface SecurityAlertData {
  id: string;
  type: 'command_blocked' | 'command_allowed' | 'approval_required' | 'security_violation' | 'security_info';
  title: string;
  message: string;
  command?: string;
  riskLevel?: 'Critical' | 'High' | 'Medium' | 'Low';
  timestamp: number;
  autoClose?: boolean;
  onAction?: () => void;
  actionLabel?: string;
}

interface SecurityAlertProps {
  alert: SecurityAlertData;
  onDismiss: (id: string) => void;
}

const getAlertConfig = (type: SecurityAlertData['type']) => {
  switch (type) {
    case 'command_blocked':
      return {
        color: 'border-red-200 bg-red-50',
        icon: ShieldAlert,
        iconColor: 'text-red-600',
        titleColor: 'text-red-800',
        textColor: 'text-red-700',
      };
    case 'command_allowed':
      return {
        color: 'border-green-200 bg-green-50',
        icon: ShieldCheck,
        iconColor: 'text-green-600',
        titleColor: 'text-green-800',
        textColor: 'text-green-700',
      };
    case 'approval_required':
      return {
        color: 'border-orange-200 bg-orange-50',
        icon: AlertTriangle,
        iconColor: 'text-orange-600',
        titleColor: 'text-orange-800',
        textColor: 'text-orange-700',
      };
    case 'security_violation':
      return {
        color: 'border-red-200 bg-red-50',
        icon: XCircle,
        iconColor: 'text-red-600',
        titleColor: 'text-red-800',
        textColor: 'text-red-700',
      };
    default:
      return {
        color: 'border-blue-200 bg-blue-50',
        icon: Shield,
        iconColor: 'text-blue-600',
        titleColor: 'text-blue-800',
        textColor: 'text-blue-700',
      };
  }
};

const getRiskBadge = (riskLevel: string) => {
  switch (riskLevel) {
    case 'Critical':
      return <Badge className="bg-red-100 text-red-800 text-xs">Critical</Badge>;
    case 'High':
      return <Badge className="bg-orange-100 text-orange-800 text-xs">High</Badge>;
    case 'Medium':
      return <Badge className="bg-yellow-100 text-yellow-800 text-xs">Medium</Badge>;
    default:
      return <Badge className="bg-blue-100 text-blue-800 text-xs">Low</Badge>;
  }
};

export function SecurityAlert({ alert, onDismiss }: SecurityAlertProps) {
  const config = getAlertConfig(alert.type);
  const Icon = config.icon;
  
  const timeAgo = React.useMemo(() => {
    const now = Date.now();
    const diff = now - alert.timestamp;
    const seconds = Math.floor(diff / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    
    if (hours > 0) return `${hours}h ago`;
    if (minutes > 0) return `${minutes}m ago`;
    return `${seconds}s ago`;
  }, [alert.timestamp]);

  return (
    <Alert className={`${config.color} relative pr-12`}>
      <Icon className={`w-4 h-4 ${config.iconColor}`} />
      
      <div className="flex-1">
        <div className="flex items-center justify-between mb-1">
          <h4 className={`font-medium text-sm ${config.titleColor}`}>
            {alert.title}
          </h4>
          
          <div className="flex items-center gap-2">
            {alert.riskLevel && getRiskBadge(alert.riskLevel)}
            <div className="flex items-center gap-1 text-xs text-gray-500">
              <Clock className="w-3 h-3" />
              <span>{timeAgo}</span>
            </div>
          </div>
        </div>
        
        <AlertDescription className={`text-sm ${config.textColor}`}>
          {alert.message}
        </AlertDescription>
        
        {alert.command && (
          <div className="mt-2">
            <code className="text-xs bg-white bg-opacity-50 px-2 py-1 rounded border font-mono">
              {alert.command.length > 60 ? `${alert.command.substring(0, 60)}...` : alert.command}
            </code>
          </div>
        )}
        
        {alert.onAction && alert.actionLabel && (
          <div className="mt-3">
            <Button
              onClick={alert.onAction}
              size="sm"
              variant="outline"
              className="text-xs h-7"
            >
              {alert.actionLabel}
            </Button>
          </div>
        )}
      </div>
      
      <Button
        onClick={() => onDismiss(alert.id)}
        variant="ghost"
        size="sm"
        className="absolute top-2 right-2 h-6 w-6 p-0 hover:bg-black hover:bg-opacity-10"
      >
        <X className="w-3 h-3" />
      </Button>
    </Alert>
  );
}

// Container for multiple alerts
interface SecurityAlertsContainerProps {
  alerts: SecurityAlertData[];
  onDismiss: (id: string) => void;
  onDismissAll?: () => void;
  maxVisible?: number;
}

export function SecurityAlertsContainer({ 
  alerts, 
  onDismiss, 
  onDismissAll,
  maxVisible = 5 
}: SecurityAlertsContainerProps) {
  const visibleAlerts = alerts.slice(0, maxVisible);
  const hiddenCount = Math.max(0, alerts.length - maxVisible);

  if (alerts.length === 0) return null;

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium text-gray-700">Security Alerts</h3>
        {alerts.length > 1 && onDismissAll && (
          <Button
            onClick={onDismissAll}
            variant="ghost"
            size="sm"
            className="text-xs h-6 px-2"
          >
            Dismiss All
          </Button>
        )}
      </div>
      
      <div className="space-y-2">
        {visibleAlerts.map((alert) => (
          <SecurityAlert
            key={alert.id}
            alert={alert}
            onDismiss={onDismiss}
          />
        ))}
        
        {hiddenCount > 0 && (
          <div className="text-xs text-gray-500 text-center py-2">
            +{hiddenCount} more alert{hiddenCount !== 1 ? 's' : ''}
          </div>
        )}
      </div>
    </div>
  );
}