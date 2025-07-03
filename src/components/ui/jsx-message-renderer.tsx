import { cn } from "@/lib/utils";
import {
  AlertCircle,
  AlertTriangle,
  Brush,
  Check,
  CheckCircle,
  Heart,
  Info,
  Lightbulb,
  Paintbrush,
  Palette,
  Rainbow,
  Sparkles,
  Star,
  ThumbsDown,
  ThumbsUp,
  X,
  XCircle,
  Zap,
} from "lucide-react";
import * as React from "react";
import { Alert, AlertDescription, AlertTitle } from "./alert";
import { Badge } from "./badge";
import { Button } from "./button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "./card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "./dialog";
import { Input } from "./input";
import { JsxRenderer } from "./jsx-renderer";
import { Label } from "./label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./select";
import { Separator } from "./separator";
import { Skeleton } from "./skeleton";
import { Switch } from "./switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./tabs";
import { Textarea } from "./textarea";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "./tooltip";

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

const StatusCard = ({
  status,
  message,
  icon,
}: {
  status: "success" | "warning" | "error" | "info";
  message: string;
  icon?: React.ReactNode;
}) => {
  const statusStyles = {
    success:
      "border-green-200 bg-green-50 text-green-800 dark:border-green-800 dark:bg-green-950 dark:text-green-200",
    warning:
      "border-yellow-200 bg-yellow-50 text-yellow-800 dark:border-yellow-800 dark:bg-yellow-950 dark:text-yellow-200",
    error:
      "border-red-200 bg-red-50 text-red-800 dark:border-red-800 dark:bg-red-950 dark:text-red-200",
    info: "border-blue-200 bg-blue-50 text-blue-800 dark:border-blue-800 dark:bg-blue-950 dark:text-blue-200",
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

const ProgressBar = ({
  progress,
  label,
}: {
  progress: number;
  label?: string;
}) => (
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

// Shape components for visual graphics
const Circle = ({
  size = 100,
  color = "blue",
  borderColor = "black",
  borderWidth = 2,
}: {
  size?: number;
  color?: string;
  borderColor?: string;
  borderWidth?: number;
}) => (
  <div className="flex flex-col items-center gap-2">
    <div
      className="rounded-full flex items-center justify-center"
      style={{
        width: `${size}px`,
        height: `${size}px`,
        backgroundColor: color,
        border: `${borderWidth}px solid ${borderColor}`,
      }}
    />
    <div className="text-sm text-gray-600 dark:text-gray-400">
      Circle: {size}×{size}px, {color} with {borderColor} border
    </div>
  </div>
);

const Rectangle = ({
  width = 100,
  height = 60,
  color = "blue",
  borderColor = "black",
  borderWidth = 2,
}: {
  width?: number;
  height?: number;
  color?: string;
  borderColor?: string;
  borderWidth?: number;
}) => (
  <div className="flex flex-col items-center gap-2">
    <div
      className="flex items-center justify-center"
      style={{
        width: `${width}px`,
        height: `${height}px`,
        backgroundColor: color,
        border: `${borderWidth}px solid ${borderColor}`,
      }}
    />
    <div className="text-sm text-gray-600 dark:text-gray-400">
      Rectangle: {width}×{height}px, {color} with {borderColor} border
    </div>
  </div>
);

const Triangle = ({
  size = 100,
  color = "blue",
  direction = "up",
}: {
  size?: number;
  color?: string;
  direction?: "up" | "down" | "left" | "right";
}) => {
  const triangleStyles = {
    up: {
      width: 0,
      height: 0,
      borderLeft: `${size / 2}px solid transparent`,
      borderRight: `${size / 2}px solid transparent`,
      borderBottom: `${size}px solid ${color}`,
    },
    down: {
      width: 0,
      height: 0,
      borderLeft: `${size / 2}px solid transparent`,
      borderRight: `${size / 2}px solid transparent`,
      borderTop: `${size}px solid ${color}`,
    },
    left: {
      width: 0,
      height: 0,
      borderTop: `${size / 2}px solid transparent`,
      borderBottom: `${size / 2}px solid transparent`,
      borderRight: `${size}px solid ${color}`,
    },
    right: {
      width: 0,
      height: 0,
      borderTop: `${size / 2}px solid transparent`,
      borderBottom: `${size / 2}px solid transparent`,
      borderLeft: `${size}px solid ${color}`,
    },
  };

  return (
    <div className="flex flex-col items-center gap-2">
      <div style={triangleStyles[direction]} />
      <div className="text-sm text-gray-600 dark:text-gray-400">
        Triangle: {size}px, {color}, pointing {direction}
      </div>
    </div>
  );
};

// Demo showcase for agent capabilities
const VisualDemo = () => (
  <Card>
    <CardHeader>
      <CardTitle className="flex items-center gap-2">
        <Sparkles className="w-5 h-5 text-blue-500" />
        JSX Visual Capabilities Demo
      </CardTitle>
      <CardDescription>
        Examples of what I can create instead of typing raw code
      </CardDescription>
    </CardHeader>
    <CardContent className="space-y-4">
      <div>
        <h4 className="text-sm font-medium mb-2">Shapes</h4>
        <div className="flex gap-4 items-center flex-wrap">
          <Circle size={60} color="lightblue" borderColor="navy" />
          <Rectangle
            width={80}
            height={60}
            color="lightgreen"
            borderColor="darkgreen"
          />
          <Triangle size={60} color="orange" direction="up" />
        </div>
      </div>

      <Separator />

      <div>
        <h4 className="text-sm font-medium mb-2">Status Cards</h4>
        <div className="space-y-2">
          <StatusCard
            status="success"
            message="Task completed successfully!"
            icon={<CheckCircle />}
          />
          <StatusCard
            status="info"
            message="Processing your request..."
            icon={<Info />}
          />
        </div>
      </div>

      <Separator />

      <div>
        <h4 className="text-sm font-medium mb-2">Progress & Colors</h4>
        <ProgressBar progress={75} label="Progress Example" />
        <div className="flex gap-2 mt-2">
          <ColorShowcase color="bg-red-500" name="Red" />
          <ColorShowcase color="bg-blue-500" name="Blue" />
          <ColorShowcase color="bg-green-500" name="Green" />
        </div>
      </div>

      <Separator />

      <Alert>
        <Lightbulb className="h-4 w-4" />
        <AlertTitle>Now I can create visuals!</AlertTitle>
        <AlertDescription>
          Instead of typing raw SVG or HTML code, I can respond with rich visual
          components that render properly in the UI.
        </AlertDescription>
      </Alert>
    </CardContent>
  </Card>
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

  // Shape components
  Circle,
  Rectangle,
  Triangle,

  // Demo component
  VisualDemo,
} as Record<string, React.ComponentType<any>>;

export function JsxMessageRenderer({
  jsx,
  className,
}: JsxMessageRendererProps) {
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
