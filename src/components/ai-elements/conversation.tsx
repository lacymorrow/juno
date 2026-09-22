"use client";

import type { ComponentProps } from "react";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { ArrowDownIcon, DownloadIcon } from "lucide-react";

/**
 * How far above the bottom still counts as "at the bottom".
 *
 * On a Retina display every scroll metric is fractional, so
 * `scrollHeight - scrollTop - clientHeight` is almost never exactly zero even
 * when the list is sitting at the end. A pixel-tight test therefore reads a
 * pinned pane as "the reader scrolled up" and autoscroll quietly dies a few
 * messages in. Roughly one line of chat is wide enough to survive the rounding
 * and still narrow enough that a deliberate scroll up unpins.
 */
const AT_BOTTOM_TOLERANCE_PX = 48;

/**
 * How long after one of our own scroll writes to keep treating scroll events as
 * our echo rather than the reader moving. A single write raises one scroll
 * event, but a tall block laying out (a streamed reply, a generated component,
 * an image) makes us write several times in quick succession, and only one flag
 * can be consumed per event — the rest then read as "the reader scrolled away"
 * and the list unpins one row in. A time window absorbs the whole burst. The
 * cost is that a genuine reader scroll within this window is ignored and they
 * simply scroll again, which is the same trade the old single-shot flag made.
 */
const SELF_SCROLL_GRACE_MS = 150;

const distanceFromBottom = (el: HTMLElement) =>
  el.scrollHeight - el.scrollTop - el.clientHeight;

interface ConversationContextValue {
  /** True while the newest message is in view; the scroll button hides on it. */
  isAtBottom: boolean;
  /** Jump to the newest message and re-arm autoscroll. */
  scrollToBottom: () => void;
  /** Callback ref for the element that actually scrolls. */
  scrollRef: (el: HTMLDivElement | null) => void;
  /** Callback ref for the message list inside the scroller. */
  contentRef: (el: HTMLDivElement | null) => void;
}

const ConversationContext = createContext<ConversationContextValue | null>(null);

/**
 * Read the conversation's scroll state. Only valid inside a `<Conversation>`.
 */
export function useConversationContext(): ConversationContextValue {
  const context = useContext(ConversationContext);
  if (!context) {
    throw new Error(
      "Conversation sub-components must be used within a <Conversation>",
    );
  }
  return context;
}

/**
 * Keeps a scroll container pinned to its newest content.
 *
 * Two things went wrong in the version this replaces, and both are answered
 * here. Scrolling used to happen once per render, which misses streamed tokens
 * and any block that lays out late (code highlighting, a tool card opening, an
 * image); this observes the list instead, so every height change re-pins.
 * And the "did the reader scroll away" test used to read scroll direction,
 * which the momentum scroller and our own programmatic writes both confuse;
 * this asks only where the list sits, which cannot drift out of sync.
 */
function useConversationAutoScroll(): ConversationContextValue {
  const scrollElement = useRef<HTMLDivElement | null>(null);
  const contentElement = useRef<HTMLDivElement | null>(null);
  const observer = useRef<ResizeObserver | null>(null);
  /** Whether new content pulls the view down. Off only once the reader scrolls away. */
  const pinned = useRef(true);
  /**
   * When we last performed a scroll ourselves (performance.now()). Scroll
   * events within SELF_SCROLL_GRACE_MS of it are our own echo, not the reader.
   *
   * This used to record the scrollTop we wrote and compare the event against
   * it. That comparison loses the race it exists to win: a streamed token
   * landing between the write and the event a frame later changes scrollHeight,
   * so the position no longer matches, our own write is read as the reader
   * scrolling away, and the list unpins mid-reply. A single boolean flag fixed
   * that but only covered one write, so a burst of writes (a tall generated
   * component laying out) still unpinned. A time window covers the whole burst.
   */
  const selfScrollAt = useRef(0);
  const [isAtBottom, setIsAtBottom] = useState(true);

  const scrollToBottom = useCallback(() => {
    const el = scrollElement.current;
    if (!el) return;
    pinned.current = true;
    selfScrollAt.current = performance.now();
    el.scrollTop = el.scrollHeight;
    setIsAtBottom(true);
  }, []);

  /** Run after layout, whenever the scroller or the list changes size. */
  const syncAfterLayout = useCallback(() => {
    const el = scrollElement.current;
    if (!el) return;

    // Nothing to scroll yet: an empty or short conversation is always "at the
    // bottom", and a cleared pane re-arms so the next reply pins again.
    if (el.scrollHeight <= el.clientHeight) {
      pinned.current = true;
      setIsAtBottom(true);
      return;
    }

    if (pinned.current) {
      scrollToBottom();
      return;
    }

    setIsAtBottom(distanceFromBottom(el) <= AT_BOTTOM_TOLERANCE_PX);
  }, [scrollToBottom]);

  const handleScroll = useCallback(() => {
    const el = scrollElement.current;
    if (!el) return;

    if (performance.now() - selfScrollAt.current < SELF_SCROLL_GRACE_MS) {
      // Our own write(s), echoed back. A tall block laying out makes several in
      // a row; the whole burst is inside the window, so stay pinned.
      return;
    }

    const atBottom = distanceFromBottom(el) <= AT_BOTTOM_TOLERANCE_PX;
    pinned.current = atBottom;
    setIsAtBottom(atBottom);
  }, []);

  const getObserver = useCallback(() => {
    if (!observer.current) {
      observer.current = new ResizeObserver(() => syncAfterLayout());
    }
    return observer.current;
  }, [syncAfterLayout]);

  // Callback refs rather than a mount effect: the list is swapped out for the
  // empty state and back, and each swap has to re-point the observer.
  const scrollRef = useCallback(
    (el: HTMLDivElement | null) => {
      const previous = scrollElement.current;
      if (previous) {
        observer.current?.unobserve(previous);
        previous.removeEventListener("scroll", handleScroll);
      }
      scrollElement.current = el;
      if (!el) return;
      el.addEventListener("scroll", handleScroll, { passive: true });
      getObserver().observe(el);
    },
    [getObserver, handleScroll],
  );

  const contentRef = useCallback(
    (el: HTMLDivElement | null) => {
      const previous = contentElement.current;
      if (previous) observer.current?.unobserve(previous);
      contentElement.current = el;
      if (!el) return;
      // A fresh list means a fresh conversation view, which opens at the newest
      // message the way Messages and Slack do.
      pinned.current = true;
      getObserver().observe(el);
    },
    [getObserver],
  );

  useEffect(
    () => () => {
      observer.current?.disconnect();
      observer.current = null;
      scrollElement.current?.removeEventListener("scroll", handleScroll);
    },
    [handleScroll],
  );

  return useMemo(
    () => ({ isAtBottom, scrollToBottom, scrollRef, contentRef }),
    [isAtBottom, scrollToBottom, scrollRef, contentRef],
  );
}

export type ConversationProps = ComponentProps<"div">;

export const Conversation = ({ className, children, ...props }: ConversationProps) => {
  const context = useConversationAutoScroll();

  return (
    <ConversationContext.Provider value={context}>
      <div
        className={cn("relative flex-1 overflow-hidden", className)}
        role="log"
        {...props}
      >
        {children}
      </div>
    </ConversationContext.Provider>
  );
};

/**
 * Force the newest message into view when the person sends one.
 *
 * Autoscroll deliberately refuses to yank somebody back down while they are
 * reading earlier messages: content arriving is not a reason to move them.
 * Sending is a different act. Its whole point is to watch what you just said
 * land and the reply form under it, so it re-arms the pin wherever the
 * scroller happened to be. Once pinned, the resize observer keeps it there
 * while the loader and the reply grow, so this fires once per send rather
 * than fighting the stream.
 *
 * `signal` should change exactly once per sent message. The id of the newest
 * user message is the natural choice; a count works too.
 */
export type ConversationScrollOnSendProps = {
  signal: string | number | null;
};

export const ConversationScrollOnSend = ({
  signal,
}: ConversationScrollOnSendProps) => {
  const { scrollToBottom } = useConversationContext();
  // Starts empty rather than at the current signal, so the first value after
  // mount counts as a send. Sending from the floating bar dispatches the query
  // and *then* opens the chat pane, so by the time this mounts the message is
  // already in the list; seeding from it meant the pane opened at the top of
  // the conversation with the new message off the bottom of the screen.
  const seen = useRef<string | number | null>(null);

  useEffect(() => {
    if (signal === null || signal === seen.current) return;
    seen.current = signal;
    scrollToBottom();
  }, [signal, scrollToBottom]);

  return null;
};

export type ConversationContentProps = ComponentProps<"div"> & {
  /** Extra classes for the scrolling element that wraps the list. */
  scrollClassName?: string;
};

export const ConversationContent = ({
  className,
  scrollClassName,
  children,
  ...props
}: ConversationContentProps) => {
  const { scrollRef, contentRef } = useConversationContext();

  return (
    <div
      ref={scrollRef}
      className={cn("h-full w-full overflow-y-auto", scrollClassName)}
      // `scroll-behavior: auto` is pinned here on purpose: a smooth scroll would
      // animate our write over several frames, so the next streamed token would
      // measure a position that is not yet at the bottom and unpin the list.
      style={{ scrollbarGutter: "stable both-edges", scrollBehavior: "auto" }}
    >
      <div
        {...props}
        ref={contentRef}
        className={cn("flex flex-col gap-8 p-4", className)}
      >
        {children}
      </div>
    </div>
  );
};

export type ConversationEmptyStateProps = ComponentProps<"div"> & {
  title?: string;
  description?: string;
  icon?: React.ReactNode;
};

export const ConversationEmptyState = ({
  className,
  title = "No messages yet",
  description = "Start a conversation to see messages here",
  icon,
  children,
  ...props
}: ConversationEmptyStateProps) => (
  <div
    className={cn(
      "flex size-full flex-col items-center justify-center gap-3 p-8 text-center",
      className
    )}
    {...props}
  >
    {children ?? (
      <>
        {icon && <div className="text-muted-foreground">{icon}</div>}
        <div className="space-y-1">
          <h3 className="font-medium text-sm">{title}</h3>
          {description && (
            <p className="text-muted-foreground text-sm">{description}</p>
          )}
        </div>
      </>
    )}
  </div>
);

export type ConversationScrollButtonProps = ComponentProps<typeof Button>;

export const ConversationScrollButton = ({
  className,
  ...props
}: ConversationScrollButtonProps) => {
  const { isAtBottom, scrollToBottom } = useConversationContext();

  return (
    !isAtBottom && (
      <Button
        className={cn(
          "absolute bottom-4 left-[50%] z-10 translate-x-[-50%] rounded-full dark:bg-background dark:hover:bg-muted",
          className
        )}
        onClick={scrollToBottom}
        size="icon"
        type="button"
        variant="outline"
        {...props}
      >
        <ArrowDownIcon className="size-4" />
      </Button>
    )
  );
};

export interface ConversationMessage {
  role: "user" | "assistant" | "system" | "data" | "tool";
  content: string;
}

export type ConversationDownloadProps = Omit<
  ComponentProps<typeof Button>,
  "onClick"
> & {
  messages: ConversationMessage[];
  filename?: string;
  formatMessage?: (message: ConversationMessage, index: number) => string;
};

const defaultFormatMessage = (message: ConversationMessage): string => {
  const roleLabel =
    message.role.charAt(0).toUpperCase() + message.role.slice(1);
  return `**${roleLabel}:** ${message.content}`;
};

export const messagesToMarkdown = (
  messages: ConversationMessage[],
  formatMessage: (
    message: ConversationMessage,
    index: number
  ) => string = defaultFormatMessage
): string => messages.map((msg, i) => formatMessage(msg, i)).join("\n\n");

export const ConversationDownload = ({
  messages,
  filename = "conversation.md",
  formatMessage = defaultFormatMessage,
  className,
  children,
  ...props
}: ConversationDownloadProps) => {
  const handleDownload = useCallback(() => {
    const markdown = messagesToMarkdown(messages, formatMessage);
    const blob = new Blob([markdown], { type: "text/markdown" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = filename;
    document.body.append(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);
  }, [messages, filename, formatMessage]);

  return (
    <Button
      className={cn(
        "absolute top-4 right-4 rounded-full dark:bg-background dark:hover:bg-muted",
        className
      )}
      onClick={handleDownload}
      size="icon"
      type="button"
      variant="outline"
      {...props}
    >
      {children ?? <DownloadIcon className="size-4" />}
    </Button>
  );
};
