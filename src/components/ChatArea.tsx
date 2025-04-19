import React from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { ChatMessage } from "@/types";

type ChatAreaProps = {
  conversation: ChatMessage[];
  conversationEndRef: React.RefObject<HTMLDivElement>;
};

const ChatArea: React.FC<ChatAreaProps> = ({ conversation, conversationEndRef }) => {
  return (
    <ScrollArea className="flex-grow mb-4 border rounded-md p-3">
      {conversation.map((msg, index) => (
        <div
          key={index}
          className={`mb-3 ${
            msg.role === "user" ? "text-right" : "text-left"
          }`}
        >
          <span
            className={cn(
              "inline-block px-3 py-1.5 rounded-lg",
              msg.role === "user"
                ? "bg-primary text-primary-foreground"
                : msg.role === "assistant"
                ? "bg-muted"
                : "bg-secondary text-secondary-foreground text-xs italic"
            )}
          >
            {msg.content}
          </span>
        </div>
      ))}
      <div ref={conversationEndRef} />
    </ScrollArea>
  );
};

export default ChatArea;