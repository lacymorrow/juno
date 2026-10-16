import {
  Actions,
  QueryButton,
  OpenButton,
  CopyButton,
} from "@/components/ui/agent-actions";
import {
  AnimatedCard,
  AnimatedNumber,
  AnimatedList,
  AnimatedProgress,
  GlowBadge,
  ShimmerText,
  Confetti,
  PulseRing,
  AnimatedDivider,
  Stat,
  Stats,
  Meter,
  MiniChart,
} from "@/components/ui/animated-components";
import {
  WeatherCard,
  FileListCard,
  SystemStatusCard,
  ComparisonCard,
  LinkCard,
  TaskSummaryCard,
  NowPlayingCard,
  AgendaCard,
  MessageCard,
  PermissionAskCard,
  ImageCard,
} from "@/components/ui/agent-cards";
import { WhyBlock } from "@/components/ui/why-block";
import {
  ComponentFallback,
  ComponentSkeleton,
} from "@/components/ui/component-skeleton";
import { stripJsxMarkup } from "@/lib/jsx-utils";
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
  TrendingUp,
  TrendingDown,
  Activity,
  Clock,
  Calendar,
  MapPin,
  Music,
  Film,
  Coffee,
  Flame,
  Bookmark,
  Globe,
  Target,
} from "lucide-react";
import * as React from "react";
import { Alert, AlertDescription, AlertTitle } from "./alert";
import { Badge } from "./badge";
import { Button } from "./button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "./card";
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "./accordion";
import { Avatar, AvatarFallback, AvatarImage } from "./avatar";
import { Checkbox } from "./checkbox";
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
import { Progress } from "./progress";
import { Separator } from "./separator";
import { Skeleton } from "./skeleton";
import { Slider } from "./slider";
import { Switch } from "./switch";
import { Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow } from "./table";
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
  /** The markup is still streaming in. */
  partial?: boolean;
  /** Top-level component name, used to size the placeholder. */
  name?: string;
}

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
  CardAction,
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
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,

  // More shadcn parts. They inherit the Mac tokens from `.agent-content`.
  Table,
  TableBody,
  TableCaption,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
  Progress,
  Avatar,
  AvatarFallback,
  AvatarImage,
  // Form controls stay registered, but the prompt does not teach them until a
  // <Form> sends their values back as the next message.
  Checkbox,
  Slider,

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

  // Domain-specific agent response cards
  WeatherCard,
  FileListCard,
  SystemStatusCard,
  ComparisonCard,
  LinkCard,
  TaskSummaryCard,
  ImageCard,
  // Live components — bound to real player state, never fake
  NowPlayingCard,
  // Events and reminders from the Mac's own apps, one shape
  AgendaCard,
  // A text or an email, as it went out (the send gate's card)
  MessageCard,
  // "I need Reminders for that." and one Continue button, before any system dialog
  PermissionAskCard,
  // Method rationale, collapsed by default (top-level <Why> is lifted out
  // before parsing; this covers a <Why> nested inside another component)
  Why: WhyBlock,

  // Animated components
  AnimatedCard,
  AnimatedNumber,
  AnimatedList,
  AnimatedProgress,
  GlowBadge,
  ShimmerText,
  Confetti,
  PulseRing,
  AnimatedDivider,
  Stat,
  Stats,
  Meter,
  MiniChart,

  // Additional icons
  TrendingUp,
  TrendingDown,
  Activity,
  Clock,
  Calendar,
  MapPin,
  Music,
  Film,
  Coffee,
  Flame,
  Bookmark,
  Globe,
  Target,

  // Interactive action components
  Actions,
  QueryButton,
  OpenButton,
  CopyButton,
} as Record<string, React.ComponentType<any>>;

interface ComponentErrorBoundaryProps {
  /** Markup being rendered; a change clears a previous failure. */
  jsx: string;
  fallback: React.ReactNode;
  children: React.ReactNode;
}

interface ComponentErrorBoundaryState {
  hasError: boolean;
}

/**
 * Catches a component that throws while rendering (bad props from the agent,
 * say) so one broken card never takes the whole conversation down. It resets
 * when the markup changes instead of being remounted, so a streaming card
 * keeps its animation state between chunks and gets another chance when more
 * markup arrives.
 */
class ComponentErrorBoundary extends React.Component<
  ComponentErrorBoundaryProps,
  ComponentErrorBoundaryState
> {
  state: ComponentErrorBoundaryState = { hasError: false };

  static getDerivedStateFromError(): ComponentErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(error: Error) {
    console.warn("Agent component failed to render:", error.message);
  }

  componentDidUpdate(prev: ComponentErrorBoundaryProps) {
    if (this.state.hasError && prev.jsx !== this.props.jsx) {
      this.setState({ hasError: false });
    }
  }

  render() {
    return this.state.hasError ? this.props.fallback : this.props.children;
  }
}

/**
 * Renders one agent component block.
 *
 * `partial` marks markup that is still streaming. If a partial block does not
 * parse yet, a skeleton holds its place; if a finished block does not parse or
 * throws, the words inside it are shown as plain text instead.
 */
export function JsxMessageRenderer({
  jsx,
  className,
  partial = false,
  name,
}: JsxMessageRendererProps) {
  const fallback = React.useMemo(
    () =>
      partial ? (
        <ComponentSkeleton name={name} />
      ) : (
        <ComponentFallback text={stripJsxMarkup(jsx)} />
      ),
    [partial, name, jsx],
  );
  const renderError = React.useCallback(() => fallback, [fallback]);

  return (
    <div className={cn("jsx-message-content agent-content", className)}>
      <ComponentErrorBoundary jsx={jsx} fallback={fallback}>
        <JsxRenderer
          jsx={jsx}
          components={availableComponents}
          fixIncompleteJsx={true}
          renderError={renderError}
        />
      </ComponentErrorBoundary>
    </div>
  );
}

// Export the available components list for documentation
export { availableComponents };
