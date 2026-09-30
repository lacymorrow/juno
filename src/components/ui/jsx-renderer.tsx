import { completeJsxTag } from "@/lib/jsx-utils";
import * as React from "react";
import JsxParser from "react-jsx-parser";

interface JsxRendererProps extends React.HTMLAttributes<HTMLDivElement> {
	jsx: string;
	fixIncompleteJsx?: boolean;
	components: Record<string, React.ComponentType>;
	/** Rendered in place of the output when the markup does not parse. */
	renderError?: (props: { error: string }) => React.ReactNode | null;
}

const JsxRenderer = React.forwardRef<JsxParser, JsxRendererProps>(
	({ className, jsx, fixIncompleteJsx = true, components, renderError }, ref) => {
		const processedJsx = React.useMemo(() => {
			return fixIncompleteJsx ? completeJsxTag(jsx) : jsx;
		}, [jsx, fixIncompleteJsx]);

		return (
			<JsxParser
				ref={ref}
				className={className}
				jsx={processedJsx}
				components={components}
				renderError={renderError}
			/>
		);
	},
);
JsxRenderer.displayName = "JsxRenderer";

export { JsxRenderer };
