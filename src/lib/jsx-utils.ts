export function matchJsxTag(code: string) {
	if (code.trim() === "") {
		return null;
	}

	const tagRegex = /<\/?([a-zA-Z][a-zA-Z0-9]*)\s*([^>]*?)(\/)?>/;
	const match = code.match(tagRegex);

	if (!match || typeof match.index === "undefined") {
		return null;
	}

	const [fullMatch, tagName, attributes, selfClosing] = match;
	const type = selfClosing
		? "self-closing"
		: fullMatch.startsWith("</")
			? "closing"
			: "opening";

	return {
		tag: fullMatch,
		tagName,
		type,
		attributes: attributes.trim(),
		startIndex: match.index,
		endIndex: match.index + fullMatch.length,
	};
}

/**
 * Completes any unclosed JSX tags in the provided code by adding their closing tags.
 * Maintains proper nesting order when adding closing tags.
 *
 * @param code - The JSX code string that may contain unclosed tags
 * @returns The completed JSX code with all necessary closing tags added
 * @example
 * completeJsxTag('<div><p);
 * // Returns: '<div></div>'
 */
export function completeJsxTag(code: string) {
	const stack: string[] = [];
	let result = "";
	let currentPosition = 0;

	while (currentPosition < code.length) {
		const match = matchJsxTag(code.slice(currentPosition));
		if (!match) {
			// No more tags — include remaining text content (important for streaming
			// where content may end mid-text, e.g. "<Card>Hello worl")
			result += code.slice(currentPosition);
			break;
		}

		const { tagName, type, endIndex } = match;

		if (type === "opening") {
			stack.push(tagName);
		} else if (type === "closing") {
			stack.pop();
		}

		result += code.slice(currentPosition, currentPosition + endIndex);
		currentPosition += endIndex;
	}

	return (
		result +
		stack
			.reverse()
			.map((tag) => `</${tag}>`)
			.join("")
	);
}

/**
 * Extracts JSX content from inside a return statement in the provided code.
 *
 * @param code - The code string containing a return statement with JSX
 * @returns The extracted JSX content as a string, or null if no content is found
 * @example
 * extractJsxContent('function Component() { return (<div>Hello</div>); }');
 * // Returns: '<div>Hello</div>'
 */
export function extractJsxContent(code: string): string | null {
	const returnContentRegex = /return\s*\(\s*([\s\S]*?)(?=\s*\);|\s*$)/;
	const match = code.match(returnContentRegex);

	if (match?.[1]) {
		return match[1].trim();
	}

	return null;
}

/**
 * Index just past the `>` that ends the opening tag starting at `start`, or -1
 * if that tag has not finished arriving. Quotes and `{...}` expressions are
 * skipped, so `data={[{a: 1}]}` or `label="a > b"` do not end the tag early.
 */
export function findOpenTagEnd(code: string, start: number): number {
	let quote: string | null = null;
	let braces = 0;
	for (let i = start + 1; i < code.length; i++) {
		const ch = code[i];
		if (quote) {
			if (ch === quote) quote = null;
			continue;
		}
		if (braces > 0) {
			if (ch === '"' || ch === "'" || ch === "`") quote = ch;
			else if (ch === "{") braces++;
			else if (ch === "}") braces--;
			continue;
		}
		if (ch === '"' || ch === "'") quote = ch;
		else if (ch === "{") braces++;
		else if (ch === ">") return i + 1;
	}
	return -1;
}

/**
 * Drop a tag that is still arriving at the very end of streamed JSX
 * (`<Card><Stat value={7`), so the part that has arrived can render without
 * the parser choking on half a tag.
 */
export function trimPartialTrailingTag(code: string): string {
	const lastOpen = code.lastIndexOf("<");
	if (lastOpen === -1) return code;
	return findOpenTagEnd(code, lastOpen) === -1 ? code.slice(0, lastOpen) : code;
}

/**
 * A component tag whose name is still arriving at the end of streamed text:
 * `<`, `</`, `<Weath`, `</Car`. The backend already holds these back; this is
 * the display-side guard for any path that does not.
 */
const UNRESOLVED_TAG_TAIL = /<\/?(?:[A-Z][A-Za-z0-9]*)?$/;

export function trimUnresolvedTagTail(text: string): string {
	return text.replace(UNRESOLVED_TAG_TAIL, "");
}

/**
 * Plain text left in a JSX fragment once every tag and `{...}` expression is
 * removed. Used when a component cannot be rendered: the person still gets
 * the words the agent wrote, never raw angle brackets.
 */
export function stripJsxMarkup(code: string): string {
	let out = "";
	let i = 0;
	while (i < code.length) {
		const ch = code[i];
		if (ch === "<" && /[A-Za-z/]/.test(code[i + 1] ?? "")) {
			const end = findOpenTagEnd(code, i);
			if (end === -1) break; // unfinished tag: nothing after it is text
			out += " ";
			i = end;
			continue;
		}
		if (ch === "{") {
			let depth = 1;
			let j = i + 1;
			while (j < code.length && depth > 0) {
				if (code[j] === "{") depth++;
				else if (code[j] === "}") depth--;
				j++;
			}
			i = j;
			continue;
		}
		out += ch;
		i++;
	}
	return out
		.split("\n")
		.map((line) => line.replace(/\s+/g, " ").trim())
		.filter(Boolean)
		.join("\n");
}
