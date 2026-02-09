import { useCallback, useEffect, useRef } from "react";
import type { ChatMessage } from "@/components/ChatMessage";

// Simple debounce function for throttling scroll
function debounce<F extends (...args: any[]) => any>(func: F, waitFor: number) {
    let timeoutId: ReturnType<typeof setTimeout> | null = null;

    return (...args: Parameters<F>): void => {
        if (timeoutId !== null) {
            clearTimeout(timeoutId);
        }
        timeoutId = setTimeout(() => func(...args), waitFor);
    };
}

interface UseChatScrollingProps {
    conversation: ChatMessage[];
    userHasScrolledUp: boolean;
    lastScrollTime: number;
    setUserHasScrolledUp: (scrolled: boolean) => void;
    setLastScrollTime: (time: number) => void;
}

export function useChatScrolling({
    conversation,
    userHasScrolledUp,
    lastScrollTime,
    setUserHasScrolledUp,
    setLastScrollTime,
}: UseChatScrollingProps) {
    const conversationEndRef = useRef<HTMLDivElement>(null);
    const scrollAreaRef = useRef<HTMLDivElement>(null);
    // Keep a ref to userHasScrolledUp so the debounced function always reads the latest value
    const userHasScrolledUpRef = useRef(userHasScrolledUp);
    userHasScrolledUpRef.current = userHasScrolledUp;

    // Improved auto-scroll function
    const autoScrollToBottom = useCallback(
        (forceScroll = false) => {
            if (!conversationEndRef.current) return;

            // Don't auto-scroll if user has scrolled up, unless forced
            if (userHasScrolledUp && !forceScroll) return;

            // Smooth scroll to bottom
            conversationEndRef.current.scrollIntoView({ behavior: "smooth" });
            setLastScrollTime(Date.now());
        },
        [userHasScrolledUp, setLastScrollTime]
    );

    // Throttled scroll function for streaming (limits frequency)
    // Uses ref to always read latest userHasScrolledUp value
    const throttledAutoScroll = useRef(
        debounce(() => {
            if (conversationEndRef.current && !userHasScrolledUpRef.current) {
                conversationEndRef.current.scrollIntoView({ behavior: "smooth" });
                setLastScrollTime(Date.now());
            }
        }, 200)
    ).current;

    // Add scroll detection
    const handleScroll = useCallback(() => {
        const scrollElement = scrollAreaRef.current?.querySelector(
            "[data-radix-scroll-area-viewport]"
        );
        if (!scrollElement) return;

        const { scrollTop, scrollHeight, clientHeight } = scrollElement;
        const scrollBottom = scrollHeight - scrollTop - clientHeight;

        // Consider user at bottom if within 100px of bottom
        const isNearBottom = scrollBottom < 100;

        // Update user scroll state
        const currentTime = Date.now();
        if (!isNearBottom && currentTime - lastScrollTime > 500) {
            // User scrolled up and it's been more than 500ms since last auto-scroll
            setUserHasScrolledUp(true);
        } else if (isNearBottom) {
            // User is at bottom, resume auto-scrolling
            setUserHasScrolledUp(false);
        }
    }, [lastScrollTime, setUserHasScrolledUp]);

    // Smart scroll conversation to bottom - only when user hasn't scrolled up
    useEffect(() => {
        autoScrollToBottom();
    }, [conversation, autoScrollToBottom]);

    // Force scroll to bottom when user sends a message or chat is cleared
    useEffect(() => {
        const lastMessage = conversation[conversation.length - 1];
        if (lastMessage?.role === "user" || conversation.length === 0) {
            // Always scroll to bottom when user sends a message or chat is empty
            autoScrollToBottom(true); // force scroll
        }
    }, [conversation, autoScrollToBottom]);

    // Add scroll event listener to detect user scroll behavior
    useEffect(() => {
        const scrollElement = scrollAreaRef.current?.querySelector(
            "[data-radix-scroll-area-viewport]"
        );
        if (!scrollElement) return;

        scrollElement.addEventListener("scroll", handleScroll);
        return () => scrollElement.removeEventListener("scroll", handleScroll);
    }, [handleScroll]);

    return {
        conversationEndRef,
        scrollAreaRef,
        autoScrollToBottom,
        throttledAutoScroll,
        handleScroll,
    };
}
