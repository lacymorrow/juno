import { completeJsxTag } from "@/lib/jsx-utils";
import * as React from "react";
import JsxParser from "react-jsx-parser";

interface JsxRendererProps extends React.HTMLAttributes<HTMLDivElement> {
  jsx: string;
  fixIncompleteJsx?: boolean;
  components: Record<string, React.ComponentType>;
}

const JsxRenderer = React.forwardRef<JsxParser, JsxRendererProps>(
  ({ className, jsx, fixIncompleteJsx = true, components }, ref) => {
    const processedJsx = React.useMemo(() => {
      // Safety check: ensure jsx is a string
      if (!jsx || typeof jsx !== "string") {
        console.warn("JsxRenderer received invalid jsx content:", jsx);
        return "<div>Invalid JSX content</div>";
      }
      return fixIncompleteJsx ? completeJsxTag(jsx) : jsx;
    }, [jsx, fixIncompleteJsx]);

    return (
      <JsxParser
        ref={ref}
        className={className}
        jsx={processedJsx}
        components={components}
      />
    );
  }
);
JsxRenderer.displayName = "JsxRenderer";

export { JsxRenderer };
