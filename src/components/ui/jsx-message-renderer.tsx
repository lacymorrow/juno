import * as React from "react";
import { JsxRenderer } from "./jsx-renderer";
import { Button } from "./button";
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "./card";
import { Alert, AlertDescription, AlertTitle } from "./alert";
import { Badge } from "./badge";
import { Input } from "./input";
import { Label } from "./label";
import { Separator } from "./separator";
import { Skeleton } from "./skeleton";
import { Switch } from "./switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./tabs";
import { Textarea } from "./textarea";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "./tooltip";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "./dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./select";
import { cn } from "@/lib/utils";
import { 
  AlertCircle, 
  Check, 
  X, 
  Star, 
  Heart, 
  ThumbsUp, 
  ThumbsDown,
  Info,
  AlertTriangle,
  CheckCircle,
  XCircle,
  Lightbulb,
  Zap,
  Sparkles,
  Palette,
  Paintbrush,
  Brush,
  Rainbow
} from "lucide-react";

interface JsxMessageRendererProps {
  jsx: string;
  className?: string;
}

// Custom showcase components for the agent to use
const ColorShowcase = ({ color, name }: { color: string; name: string }) => (
  <div className="flex items-center gap-2 p-2 rounded border">
    <div className={`w-6 h-6 rounded-full ${color}`} />
    <span className="text-sm font-medium">{name}</span>
  </div>
);

const StatusCard = ({ status, message, icon }: { status: "success" | "warning" | "error" | "info"; message: string; icon?: React.ReactNode }) => {
  const statusStyles = {
    success: "border-green-200 bg-green-50 text-green-800 dark:border-green-800 dark:bg-green-950 dark:text-green-200",
    warning: "border-yellow-200 bg-yellow-50 text-yellow-800 dark:border-yellow-800 dark:bg-yellow-950 dark:text-yellow-200", 
    error: "border-red-200 bg-red-50 text-red-800 dark:border-red-800 dark:bg-red-950 dark:text-red-200",
    info: "border-blue-200 bg-blue-50 text-blue-800 dark:border-blue-800 dark:bg-blue-950 dark:text-blue-200"
  };

  return (
    <div className={`p-3 rounded-lg border ${statusStyles[status]}`}>
      <div className="flex items-center gap-2">
        {icon}
        <span className="font-medium">{message}</span>
      </div>
    </div>
  );
};

const ProgressBar = ({ progress, label }: { progress: number; label?: string }) => (
  <div className="space-y-2">
    {label && <div className="text-sm font-medium">{label}</div>}
    <div className="w-full bg-gray-200 rounded-full h-2 dark:bg-gray-700">
      <div 
        className="bg-blue-600 h-2 rounded-full transition-all duration-300" 
        style={{ width: `${Math.min(100, Math.max(0, progress))}%` }}
      />
    </div>
    <div className="text-xs text-gray-600 dark:text-gray-400">{progress}%</div>
  </div>
);

// Available components for the agent to use
const availableComponents = {
  // Layout & Structure
  div: (props: React.HTMLAttributes<HTMLDivElement>) => <div {...props} />,
  span: (props: React.HTMLAttributes<HTMLSpanElement>) => <span {...props} />,
  p: (props: React.HTMLAttributes<HTMLParagraphElement>) => <p {...props} />,
  h1: (props: React.HTMLAttributes<HTMLHeadingElement>) => <h1 {...props} />,
  h2: (props: React.HTMLAttributes<HTMLHeadingElement>) => <h2 {...props} />,
  h3: (props: React.HTMLAttributes<HTMLHeadingElement>) => <h3 {...props} />,
  h4: (props: React.HTMLAttributes<HTMLHeadingElement>) => <h4 {...props} />,
  h5: (props: React.HTMLAttributes<HTMLHeadingElement>) => <h5 {...props} />,
  h6: (props: React.HTMLAttributes<HTMLHeadingElement>) => <h6 {...props} />,
  
  // UI Components
  Button,
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
  Alert,
  AlertDescription,
  AlertTitle,
  Badge,
  Input,
  Label,
  Separator,
  Skeleton,
  Switch,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  Textarea,
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  
  // Custom showcase components
  ColorShowcase,
  StatusCard,
  ProgressBar,
  
  // Icons
  AlertCircle,
  Check,
  X,
  Star,
  Heart,
  ThumbsUp,
  ThumbsDown,
  Info,
  AlertTriangle,
  CheckCircle,
  XCircle,
  Lightbulb,
  Zap,
  Sparkles,
  Palette,
  Paintbrush,
  Brush,
  Rainbow,
} as Record<string, React.ComponentType<any>>;

// Helper function to detect if content contains JSX
export function isJsxContent(content: string): boolean {
  // Check for JSX-like content
  const jsxPattern = /<[A-Z][A-Za-z0-9]*\s*[^>]*>|<\/[A-Z][A-Za-z0-9]*>/;
  return jsxPattern.test(content.trim());
}

export function JsxMessageRenderer({ jsx, className }: JsxMessageRendererProps) {
  return (
    <div className={cn("jsx-message-content", className)}>
      <JsxRenderer
        jsx={jsx}
        components={availableComponents}
        fixIncompleteJsx={true}
      />
    </div>
  );
}

// Export the available components list for documentation
export { availableComponents };