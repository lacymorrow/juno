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

  // Extract MARKDOWN_CONTENT sections
  const markdownRegex = /<!--\s*MARKDOWN_CONTENT\s*-->([\s\S]*?)<!--\s*\/MARKDOWN_CONTENT\s*-->/gi;
  const markdownMatches = content.match(markdownRegex);
  if (markdownMatches) {
    result.markdown = markdownMatches
      .map(match => {
        const contentMatch = match.match(/<!--\s*MARKDOWN_CONTENT\s*-->([\s\S]*?)<!--\s*\/MARKDOWN_CONTENT\s*-->/i);
        return contentMatch ? contentMatch[1].trim() : '';
      })
      .filter(Boolean)
      .join('\n\n');
    result.hasStructuredContent = true;
  }

  // Extract VISUAL_CONTENT (JSX) sections
  const visualRegex = /\{\s*\/\*\s*VISUAL_CONTENT\s*\*\/\s*\}([\s\S]*?)\{\s*\/\*\s*\/VISUAL_CONTENT\s*\*\/\s*\}/gi;
  const visualMatches = content.match(visualRegex);
  if (visualMatches) {
    result.visual = visualMatches
      .map(match => {
        const contentMatch = match.match(/\{\s*\/\*\s*VISUAL_CONTENT\s*\*\/\s*\}([\s\S]*?)\{\s*\/\*\s*\/VISUAL_CONTENT\s*\*\/\s*\}/i);
        return contentMatch ? contentMatch[1].trim() : '';
      })
      .filter(Boolean)
      .join('\n');
    result.hasStructuredContent = true;
  }

  // Extract SPEECH_CONTENT sections
  const speechRegex = /<!--\s*SPEECH_CONTENT\s*-->([\s\S]*?)<!--\s*\/SPEECH_CONTENT\s*-->/gi;
  const speechMatches = content.match(speechRegex);
  if (speechMatches) {
    result.speech = speechMatches
      .map(match => {
        const contentMatch = match.match(/<!--\s*SPEECH_CONTENT\s*-->([\s\S]*?)<!--\s*\/SPEECH_CONTENT\s*-->/i);
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
  
  const markdownPattern = /<!--\s*MARKDOWN_CONTENT\s*-->/i;
  const visualPattern = /\{\s*\/\*\s*VISUAL_CONTENT\s*\*\/\s*\}/i;
  const speechPattern = /<!--\s*SPEECH_CONTENT\s*-->/i;
  
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
    // Remove markdown markers
    .replace(/<!--\s*MARKDOWN_CONTENT\s*-->/gi, '')
    .replace(/<!--\s*\/MARKDOWN_CONTENT\s*-->/gi, '')
    // Remove visual markers
    .replace(/\{\s*\/\*\s*VISUAL_CONTENT\s*\*\/\s*\}/gi, '')
    .replace(/\{\s*\/\*\s*\/VISUAL_CONTENT\s*\*\/\s*\}/gi, '')
    // Remove speech markers
    .replace(/<!--\s*SPEECH_CONTENT\s*-->/gi, '')
    .replace(/<!--\s*\/SPEECH_CONTENT\s*-->/gi, '')
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