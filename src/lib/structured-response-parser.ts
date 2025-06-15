export interface StructuredResponse {
  markdown?: string;
  jsx?: string; 
  tts?: string;
  hasStructuredContent: boolean;
}

/**
 * Parses structured agent responses that contain multiple content formats.
 * Looks for format markers in agent responses for different content types.
 */
export function parseStructuredResponse(content: string): StructuredResponse {
  const result: StructuredResponse = {
    hasStructuredContent: false
  };

  // Check if content contains structured format markers
  const hasMarkdownMarker = content.includes('```markdown <!-- MD -->');
  const hasJsxMarker = content.includes('```jsx {/* VIS */}');
  const hasTtsMarker = content.includes('```text <!-- TTS -->');

  if (!hasMarkdownMarker && !hasJsxMarker && !hasTtsMarker) {
    // No structured content, return as-is
    return result;
  }

  result.hasStructuredContent = true;

  // Extract markdown section
  if (hasMarkdownMarker) {
    const markdownRegex = /```markdown <!-- MD -->\s*([\s\S]*?)\s*```/;
    const markdownMatch = content.match(markdownRegex);
    if (markdownMatch && markdownMatch[1]) {
      result.markdown = markdownMatch[1].trim();
    }
  }

  // Extract JSX section
  if (hasJsxMarker) {
    const jsxRegex = /```jsx \{\/\* VIS \*\/\}\s*([\s\S]*?)\s*```/;
    const jsxMatch = content.match(jsxRegex);
    if (jsxMatch && jsxMatch[1]) {
      result.jsx = jsxMatch[1].trim();
    }
  }

  // Extract TTS section
  if (hasTtsMarker) {
    const ttsRegex = /```text <!-- TTS -->\s*([\s\S]*?)\s*```/;
    const ttsMatch = content.match(ttsRegex);
    if (ttsMatch && ttsMatch[1]) {
      result.tts = ttsMatch[1].trim();
    }
  }

  return result;
}

/**
 * Determines what type of content to display based on structured response
 * Priority: JSX > Markdown > TTS > Original content
 */
export function getDisplayContent(content: string, parsedResponse?: StructuredResponse): {
  displayContent: string;
  contentType: 'jsx' | 'markdown' | 'text';
  isJsx: boolean;
} {
  const parsed = parsedResponse || parseStructuredResponse(content);
  
  if (parsed.hasStructuredContent) {
    // Priority: JSX first for visual components
    if (parsed.jsx) {
      return {
        displayContent: parsed.jsx,
        contentType: 'jsx',
        isJsx: true
      };
    }
    
    // Then markdown for rich text
    if (parsed.markdown) {
      return {
        displayContent: parsed.markdown,
        contentType: 'markdown', 
        isJsx: false
      };
    }
    
    // Finally TTS text as fallback
    if (parsed.tts) {
      return {
        displayContent: parsed.tts,
        contentType: 'text',
        isJsx: false
      };
    }
  }
  
  // Fallback to original content with simple JSX detection
  const isJsxContent = content.includes('<') && content.includes('>') && (
    content.includes('Card') ||
    content.includes('Alert') ||
    content.includes('Button') ||
    content.includes('Badge') ||
    content.includes('Circle') ||
    content.includes('Rectangle') ||
    content.includes('Triangle') ||
    content.includes('StatusCard') ||
    content.includes('ColorShowcase') ||
    content.includes('VisualDemo') ||
    content.includes('className=') ||
    content.includes('jsx') ||
    content.includes('React')
  );
  
  return {
    displayContent: content,
    contentType: isJsxContent ? 'jsx' : 'text',
    isJsx: isJsxContent
  };
}

/**
 * Gets the best content for text-to-speech
 * Priority: TTS > Markdown > JSX (stripped) > Original content
 */
export function getTTSContent(content: string, parsedResponse?: StructuredResponse): string {
  const parsed = parsedResponse || parseStructuredResponse(content);
  
  if (parsed.hasStructuredContent) {
    // Use dedicated TTS content if available
    if (parsed.tts) {
      return parsed.tts;
    }
    
    // Use markdown content (readable for TTS)
    if (parsed.markdown) {
      return parsed.markdown;
    }
    
    // Strip JSX tags for TTS if only JSX is available
    if (parsed.jsx) {
      return parsed.jsx.replace(/<[^>]*>/g, ' ').replace(/\s+/g, ' ').trim();
    }
  }
  
  // Fallback to original content, strip JSX tags if present
  if (content.includes('<') && content.includes('>')) {
    return content.replace(/<[^>]*>/g, ' ').replace(/\s+/g, ' ').trim();
  }
  
  return content;
}