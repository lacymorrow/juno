export interface StructuredResponse {
  markdown?: string;
  visual?: string;
  speech?: string;
  hasStructuredContent: boolean;
  originalContent: string;
}

/**
 * Parses agent responses for structured content sections.
 * Extracts MARKDOWN_CONTENT, VISUAL_CONTENT, and SPEECH_CONTENT sections.
 *
 * @param content - The raw response content from the agent
 * @returns Parsed structured response with separate content types
 */
export function parseStructuredResponse(content: string): StructuredResponse {
  if (!content || content.trim() === '') {
    return {
      hasStructuredContent: false,
      originalContent: content,
    };
  }

  const result: StructuredResponse = {
    hasStructuredContent: false,
    originalContent: content,
  };

  // Extract markdown sections (<!-- MD --> format)
  const markdownRegex = /```markdown\s*<!--\s*MD\s*-->([\s\S]*?)```/gi;
  const markdownMatches = content.match(markdownRegex);
  if (markdownMatches) {
    result.markdown = markdownMatches
      .map(match => {
        const contentMatch = match.match(/```markdown\s*<!--\s*MD\s*-->([\s\S]*?)```/i);
        return contentMatch ? contentMatch[1].trim() : '';
      })
      .filter(Boolean)
      .join('\n\n');
    result.hasStructuredContent = true;
  }

  // Extract visual/JSX sections ({/* VIS */} format)
  const visualRegex = /```jsx\s*\{\s*\/\*\s*VIS\s*\*\/\s*\}([\s\S]*?)```/gi;
  const visualMatches = content.match(visualRegex);
  if (visualMatches) {
    result.visual = visualMatches
      .map(match => {
        const contentMatch = match.match(/```jsx\s*\{\s*\/\*\s*VIS\s*\*\/\s*\}([\s\S]*?)```/i);
        return contentMatch ? contentMatch[1].trim() : '';
      })
      .filter(Boolean)
      .join('\n');
    result.hasStructuredContent = true;
  }

  // Extract speech/TTS sections (<!-- TTS --> format)
  const speechRegex = /```text\s*<!--\s*TTS\s*-->([\s\S]*?)```/gi;
  const speechMatches = content.match(speechRegex);
  if (speechMatches) {
    result.speech = speechMatches
      .map(match => {
        const contentMatch = match.match(/```text\s*<!--\s*TTS\s*-->([\s\S]*?)```/i);
        return contentMatch ? contentMatch[1].trim() : '';
      })
      .filter(Boolean)
      .join(' ');
    result.hasStructuredContent = true;
  }

  return result;
}

/**
 * Checks if content contains structured response sections.
 *
 * @param content - The content to check
 * @returns True if content has structured sections
 */
export function hasStructuredContent(content: string): boolean {
  if (!content) return false;

  const markdownPattern = /```markdown\s*<!--\s*MD\s*-->/i;
  const visualPattern = /```jsx\s*\{\s*\/\*\s*VIS\s*\*\/\s*\}/i;
  const speechPattern = /```text\s*<!--\s*TTS\s*-->/i;

  return markdownPattern.test(content) ||
    visualPattern.test(content) ||
    speechPattern.test(content);
}

/**
 * Gets the appropriate content type for rendering based on parsed content.
 * Priority: visual > markdown > speech > original
 *
 * @param parsed - The parsed structured response
 * @returns Object with content and type for rendering
 */
export function getRenderContent(parsed: StructuredResponse): {
  content: string;
  type: 'jsx' | 'markdown' | 'text';
  speechText?: string;
} {
  // For TTS, always use speech content if available, otherwise use original
  let speechText = parsed.speech || parsed.originalContent;

  // For rendering, prioritize visual content
  if (parsed.visual) {
    return {
      content: parsed.visual,
      type: 'jsx',
      speechText,
    };
  }

  // Then markdown content
  if (parsed.markdown) {
    return {
      content: parsed.markdown,
      type: 'markdown',
      speechText,
    };
  }

  // Fall back to original content
  return {
    content: parsed.originalContent,
    type: 'text',
    speechText,
  };
}

/**
 * Removes structured content markers from text, leaving only the content.
 * Useful for cleaning up content when structured parsing isn't needed.
 *
 * @param content - The content with potential structured markers
 * @returns Clean content without markers
 */
export function cleanStructuredMarkers(content: string): string {
  if (!content) return content;

  return content
    // Remove markdown code blocks with markers
    .replace(/```markdown\s*<!--\s*MD\s*-->/gi, '')
    .replace(/```(?=\s*```jsx)/gi, '') // Remove closing markdown before jsx
    // Remove jsx code blocks with markers
    .replace(/```jsx\s*\{\s*\/\*\s*VIS\s*\*\/\s*\}/gi, '')
    .replace(/```(?=\s*```text)/gi, '') // Remove closing jsx before text
    // Remove text code blocks with markers
    .replace(/```text\s*<!--\s*TTS\s*-->/gi, '')
    .replace(/```$/gm, '') // Remove trailing code block markers
    // Clean up extra whitespace
    .replace(/\n\s*\n\s*\n/g, '\n\n')
    .trim();
}

/**
 * Combines multiple content sections for comprehensive display.
 * Useful for showing all content when needed.
 *
 * @param parsed - The parsed structured response
 * @returns Combined content for display
 */
export function getCombinedContent(parsed: StructuredResponse): string {
  const sections: string[] = [];

  if (parsed.markdown) {
    sections.push(`**Documentation:**\n${parsed.markdown}`);
  }

  if (parsed.visual) {
    sections.push(`**Visual Components:**\n\`\`\`jsx\n${parsed.visual}\n\`\`\``);
  }

  if (parsed.speech) {
    sections.push(`**Speech Text:**\n${parsed.speech}`);
  }

  if (sections.length === 0) {
    return parsed.originalContent;
  }

  return sections.join('\n\n---\n\n');
}
