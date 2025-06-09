import React, { useState, useEffect } from 'react';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from './ui/dialog';
import { Button } from './ui/button';
import { Badge } from './ui/badge';
import { Alert, AlertDescription } from './ui/alert';
import { Card, CardContent, CardHeader, CardTitle } from './ui/card';
import { Shield, ShieldAlert, ShieldCheck, Clock, Terminal, AlertTriangle } from 'lucide-react';

interface SecurityApprovalRequest {
  id: string;
  command: string;
  tool_name: string;
  description: string;
  risk_level: 'Critical' | 'High' | 'Medium' | 'Low';
  requested_at: number;
  timeout_at: number;
}

interface SecurityApprovalModalProps {
  isOpen: boolean;
  request: SecurityApprovalRequest | null;
  onApprove: (decision: 'allow_once' | 'allow_always') => void;
  onDeny: (decision: 'deny_once' | 'deny_always') => void;
  onClose: () => void;
}

const getRiskConfig = (riskLevel: string) => {
  switch (riskLevel) {
    case 'Critical':
      return {
        color: 'bg-red-100 text-red-800 border-red-200',
        icon: ShieldAlert,
        iconColor: 'text-red-600',
        description: 'This command could cause severe system damage',
      };
    case 'High':
      return {
        color: 'bg-orange-100 text-orange-800 border-orange-200',
        icon: AlertTriangle,
        iconColor: 'text-orange-600',
        description: 'This command requires elevated privileges',
      };
    case 'Medium':
      return {
        color: 'bg-yellow-100 text-yellow-800 border-yellow-200',
        icon: Shield,
        iconColor: 'text-yellow-600',
        description: 'This command performs system operations',
      };
    default:
      return {
        color: 'bg-blue-100 text-blue-800 border-blue-200',
        icon: ShieldCheck,
        iconColor: 'text-blue-600',
        description: 'This command is generally safe',
      };
  }
};

export function SecurityApprovalModal({
  isOpen,
  request,
  onApprove,
  onDeny,
  onClose,
}: SecurityApprovalModalProps) {
  const [timeRemaining, setTimeRemaining] = useState<number>(0);

  useEffect(() => {
    if (!request) return;

    const updateTimer = () => {
      const now = Date.now() / 1000;
      const remaining = Math.max(0, request.timeout_at - now);
      setTimeRemaining(remaining);

      if (remaining <= 0) {
        onDeny('deny_once'); // Auto-deny on timeout
      }
    };

    updateTimer();
    const interval = setInterval(updateTimer, 1000);

    return () => clearInterval(interval);
  }, [request, onDeny]);

  if (!request) return null;

  const riskConfig = getRiskConfig(request.risk_level);
  const RiskIcon = riskConfig.icon;
  const minutes = Math.floor(timeRemaining / 60);
  const seconds = Math.floor(timeRemaining % 60);

  return (
    <Dialog open={isOpen} onOpenChange={onClose}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Shield className="w-5 h-5" />
            Security Approval Required
          </DialogTitle>
          <DialogDescription>
            The AI agent is requesting permission to execute a command
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          {/* Risk Level Badge */}
          <div className="flex items-center justify-between">
            <Badge className={`${riskConfig.color} flex items-center gap-1`}>
              <RiskIcon className={`w-4 h-4 ${riskConfig.iconColor}`} />
              {request.risk_level} Risk
            </Badge>
            
            <div className="flex items-center gap-1 text-sm text-gray-600">
              <Clock className="w-4 h-4" />
              {minutes}:{seconds.toString().padStart(2, '0')} remaining
            </div>
          </div>

          {/* Command Details */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2 text-base">
                <Terminal className="w-4 h-4" />
                Command Details
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              <div>
                <label className="text-sm font-medium text-gray-700">Command:</label>
                <code className="block mt-1 p-2 bg-gray-100 rounded text-sm font-mono break-all">
                  {request.command}
                </code>
              </div>
              
              <div>
                <label className="text-sm font-medium text-gray-700">Tool:</label>
                <span className="ml-2 text-sm">{request.tool_name}</span>
              </div>
              
              <div>
                <label className="text-sm font-medium text-gray-700">Description:</label>
                <span className="ml-2 text-sm">{request.description}</span>
              </div>
            </CardContent>
          </Card>

          {/* Risk Warning */}
          <Alert className={`border ${riskConfig.color.split(' ')[2]}`}>
            <RiskIcon className={`w-4 h-4 ${riskConfig.iconColor}`} />
            <AlertDescription className="font-medium">
              {riskConfig.description}
            </AlertDescription>
          </Alert>

          {/* Action Buttons */}
          <div className="flex flex-col sm:flex-row gap-2 pt-4">
            <div className="flex-1 space-y-2">
              <h4 className="text-sm font-medium text-green-700">Allow</h4>
              <div className="flex gap-2">
                <Button
                  onClick={() => onApprove('allow_once')}
                  variant="outline"
                  className="flex-1 border-green-200 text-green-700 hover:bg-green-50"
                >
                  Allow Once
                </Button>
                <Button
                  onClick={() => onApprove('allow_always')}
                  variant="outline"
                  className="flex-1 border-green-200 text-green-700 hover:bg-green-50"
                >
                  Always Allow
                </Button>
              </div>
            </div>

            <div className="flex-1 space-y-2">
              <h4 className="text-sm font-medium text-red-700">Deny</h4>
              <div className="flex gap-2">
                <Button
                  onClick={() => onDeny('deny_once')}
                  variant="outline"
                  className="flex-1 border-red-200 text-red-700 hover:bg-red-50"
                >
                  Deny Once
                </Button>
                <Button
                  onClick={() => onDeny('deny_always')}
                  variant="outline"
                  className="flex-1 border-red-200 text-red-700 hover:bg-red-50"
                >
                  Always Deny
                </Button>
              </div>
            </div>
          </div>

          {/* Auto-timeout warning */}
          {timeRemaining < 30 && (
            <Alert className="border-orange-200 bg-orange-50">
              <AlertTriangle className="w-4 h-4 text-orange-600" />
              <AlertDescription className="text-orange-800">
                Command will be automatically denied in {Math.ceil(timeRemaining)} seconds
              </AlertDescription>
            </Alert>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}