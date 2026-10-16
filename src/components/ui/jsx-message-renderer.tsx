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
  Field,
  Form,
  FormCheckbox,
  FormInput,
  FormSelect,
  FormSelectContent,
  FormSelectItem,
  FormSelectTrigger,
  FormSelectValue,
  FormSlider,
  FormSwitch,
  FormTextarea,
} from "@/components/ui/agent-form";
import {
  WeatherCard,
  FileListCard,
  MetricList,
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
import { filterAgentClassName, setsSize } from "@/lib/agent-classnames";
import { describeFallback } from "@/lib/jsx-utils";
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
import { JsxRenderer } from "./jsx-renderer";
import { Label } from "./label";
import { Progress } from "./progress";
import { Separator } from "./separator";
import { Skeleton } from "./skeleton";
import { Table, TableBody, TableCaption, TableCell, TableHead, TableHeader, TableRow } from "./table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./tabs";
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
const registry = {
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
  Label,
  Separator,
  Skeleton,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,

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
  // Form controls. They draw only inside a <Form>, which sends their values
  // back as the next message; alone they would be bound to nothing.
  Form,
  Field,
  Input: FormInput,
  Textarea: FormTextarea,
  Switch: FormSwitch,
  Checkbox: FormCheckbox,
  Slider: FormSlider,
  Select: FormSelect,
  SelectContent: FormSelectContent,
  SelectItem: FormSelectItem,
  SelectTrigger: FormSelectTrigger,
  SelectValue: FormSelectValue,

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
  MetricList,
  // The earlier tag for MetricList, kept for one prompt cycle
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

/** Registry entries that are icons: drawn small and quiet, whatever the agent asks. */
const ICON_NAMES = new Set([
  "AlertCircle", "Check", "X", "Star", "Heart", "ThumbsUp", "ThumbsDown", "Info", "AlertTriangle",
  "CheckCircle", "XCircle", "Lightbulb", "Zap", "Sparkles", "Palette", "Paintbrush", "Brush", "Rainbow",
  "TrendingUp", "TrendingDown", "Activity", "Clock", "Calendar", "MapPin", "Music", "Film", "Coffee",
  "Flame", "Bookmark", "Globe", "Target",
]);

/** Drops colour, gradient and label-style classes the agent writes (see `agent-classnames.ts`). */
function withClassFilter(Component: React.ComponentType<any>): React.ComponentType<any> {
  const Filtered = (props: { className?: string }) => (
    <Component {...props} className={filterAgentClassName(props.className)} />
  );
  // Statics the form layout reads (see `Field`).
  Object.assign(Filtered, { inlineControl: (Component as { inlineControl?: boolean }).inlineControl });
  Filtered.displayName = `Agent(${(Component as { displayName?: string }).displayName ?? Component.name ?? "Component"})`;
  return Filtered;
}

/** Icons are 14px in the secondary label colour unless the agent sizes them. */
function withIconDefaults(Icon: React.ComponentType<any>): React.ComponentType<any> {
  const Quiet = (props: { className?: string }) => {
    const className = filterAgentClassName(props.className);
    return <Icon {...props} className={cn(!setsSize(className) && "size-3.5", "text-muted-foreground", className)} />;
  };
  Quiet.displayName = `AgentIcon(${(Icon as { displayName?: string }).displayName ?? "Icon"})`;
  return Quiet;
}

const availableComponents: Record<string, React.ComponentType<any>> = Object.fromEntries(
  Object.entries(registry).map(([name, Component]) => [
    name,
    ICON_NAMES.has(name) ? withIconDefaults(Component) : withClassFilter(Component),
  ]),
);

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
        <ComponentFallback text={describeFallback(jsx)} />
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
