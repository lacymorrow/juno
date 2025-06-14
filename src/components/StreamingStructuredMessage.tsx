import { AIResponse } from "@/components/ui/kibo-ui/ai";
import { JsxMessageRenderer } from "@/components/ui/jsx-message-renderer";
import { StreamingSections } from "@/lib/streaming-structured-parser";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";

interface StreamingStructuredMessageProps {
  sections: StreamingSections;
  isStreaming: boolean;
}

export function StreamingStructuredMessage({
  sections,
  isStreaming,
}: StreamingStructuredMessageProps) {
  // If no structured content yet, show the default content
  if (!sections.hasStructuredContent) {
    return <AIResponse>{sections.default.content}</AIResponse>;
  }

  return (
    <div className="space-y-4">
      {/* Show active section indicator if streaming */}
      {isStreaming && (
        <div className="flex items-center gap-2">
          <Badge variant="outline" className="text-xs">
            Streaming {sections.currentSection}
          </Badge>
          <div className="h-2 w-2 bg-green-500 rounded-full animate-pulse" />
        </div>
      )}

      {/* Render sections based on content availability */}

      {/* Visual section - highest priority */}
      {sections.visual.content.trim() && (
        <div>
          <JsxMessageRenderer jsx={sections.visual.content} />
          {sections.markdown.content.trim() && <Separator className="my-4" />}
        </div>
      )}

      {/* Markdown section */}
      {sections.markdown.content.trim() && (
        <div>
          <AIResponse>{sections.markdown.content}</AIResponse>
          {sections.speech.content.trim() && sections.visual.content.trim() && (
            <Separator className="my-4" />
          )}
        </div>
      )}

      {/* Speech section - only show if no visual content */}
      {sections.speech.content.trim() && !sections.visual.content.trim() && (
        <AIResponse>{sections.speech.content}</AIResponse>
      )}

      {/* Fallback to default if no other content */}
      {!sections.visual.content.trim() &&
        !sections.markdown.content.trim() &&
        !sections.speech.content.trim() &&
        sections.default.content.trim() && (
          <AIResponse>{sections.default.content}</AIResponse>
        )}
    </div>
  );
}
