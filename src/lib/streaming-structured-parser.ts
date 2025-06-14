// Streaming structured response parser
// Handles real-time parsing of structured response chunks during streaming

export interface StreamingSection {
    type: 'markdown' | 'visual' | 'speech' | 'default';
    content: string;
    isComplete: boolean;
}

export interface StreamingSections {
    markdown: StreamingSection;
    visual: StreamingSection;
    speech: StreamingSection;
    default: StreamingSection;
    currentSection: 'markdown' | 'visual' | 'speech' | 'default';
    hasStructuredContent: boolean;
}

// Section start markers - designed to be detected at chunk boundaries
export const SECTION_MARKERS = {
    MARKDOWN_START: '<!-- MD -->',
    VISUAL_START: '{/* VIS */',
    SPEECH_START: '<!-- TTS -->',
} as const;

// Regex patterns for detecting section markers
const MARKER_PATTERNS = {
    MARKDOWN: /<!-- MD -->/,
    VISUAL: /\{\/\* VIS \*\//,
    SPEECH: /<!-- TTS -->/,
} as const;

export class StreamingStructuredParser {
    private sections: StreamingSections;
    private buffer: string = '';
    private lastMarkerPosition: number = 0;

    constructor() {
        this.sections = this.createEmptySections();
    }

    private createEmptySections(): StreamingSections {
        const createSection = (type: StreamingSection['type']): StreamingSection => ({
            type,
            content: '',
            isComplete: false,
        });

        return {
            markdown: createSection('markdown'),
            visual: createSection('visual'),
            speech: createSection('speech'),
            default: createSection('default'),
            currentSection: 'default',
            hasStructuredContent: false,
        };
    }

    /**
     * Process a new chunk from the streaming response
     * Returns the sections that were updated in this chunk
     */
    processChunk(chunk: string): {
        updatedSections: (keyof StreamingSections)[];
        sections: StreamingSections;
    } {
        this.buffer += chunk;
        const updatedSections: (keyof StreamingSections)[] = [];

        // Look for section markers in the accumulated buffer
        const markerChecks = [
            { pattern: MARKER_PATTERNS.MARKDOWN, type: 'markdown' as const },
            { pattern: MARKER_PATTERNS.VISUAL, type: 'visual' as const },
            { pattern: MARKER_PATTERNS.SPEECH, type: 'speech' as const },
        ];

        // Check if we've encountered any section markers
        for (const { pattern, type } of markerChecks) {
            const match = this.buffer.match(pattern);
            if (match && match.index !== undefined) {
                // Found a section marker
                if (!this.sections.hasStructuredContent) {
                    this.sections.hasStructuredContent = true;
                }

                // If we're transitioning from another section, mark it as complete
                if (this.sections.currentSection !== 'default' && this.sections.currentSection !== type) {
                    this.sections[this.sections.currentSection].isComplete = true;
                }

                // Switch to the new section
                this.sections.currentSection = type;

                // Extract content after the marker
                const markerEnd = match.index + match[0].length;
                const contentAfterMarker = this.buffer.slice(markerEnd);

                // Add content to the appropriate section
                this.sections[type].content += contentAfterMarker;
                this.sections[type].isComplete = false;

                updatedSections.push(type);

                // Update buffer to remove processed content
                this.buffer = '';
                break;
            }
        }

        // If no new markers found, add chunk to current section
        if (updatedSections.length === 0) {
            const currentSection = this.sections.currentSection;

            // For default section, only add if we haven't found structured content yet
            if (currentSection === 'default' && !this.sections.hasStructuredContent) {
                this.sections.default.content += chunk;
                updatedSections.push('default');
            } else if (currentSection !== 'default') {
                // Add to the current structured section
                this.sections[currentSection].content += chunk;
                updatedSections.push(currentSection);
            }
        }

        return {
            updatedSections,
            sections: { ...this.sections },
        };
    }

    /**
     * Mark the streaming as complete and finalize all sections
     */
    finalize(): StreamingSections {
        // Mark current section as complete
        if (this.sections.currentSection !== 'default') {
            this.sections[this.sections.currentSection].isComplete = true;
        } else {
            this.sections.default.isComplete = true;
        }

        // Clean up any remaining buffer content
        if (this.buffer.trim()) {
            const currentSection = this.sections.currentSection;
            this.sections[currentSection].content += this.buffer;
        }

        return { ...this.sections };
    }

    /**
     * Get the current state of all sections
     */
    getCurrentSections(): StreamingSections {
        return { ...this.sections };
    }

    /**
     * Reset the parser for a new streaming session
     */
    reset(): void {
        this.sections = this.createEmptySections();
        this.buffer = '';
        this.lastMarkerPosition = 0;
    }

    /**
     * Get content for rendering - prioritizes visual, then markdown, then default
     */
    getRenderContent(): { content: string; type: StreamingSection['type'] } {
        if (this.sections.visual.content.trim()) {
            return { content: this.sections.visual.content, type: 'visual' };
        }
        if (this.sections.markdown.content.trim()) {
            return { content: this.sections.markdown.content, type: 'markdown' };
        }
        return { content: this.sections.default.content, type: 'default' };
    }

    /**
     * Get content for TTS - prioritizes speech, then falls back to render content
     */
    getTTSContent(): string {
        if (this.sections.speech.content.trim()) {
            return this.sections.speech.content.trim();
        }

        // Fall back to render content, but clean it for TTS
        const renderContent = this.getRenderContent().content;
        return this.cleanForTTS(renderContent);
    }

    /**
     * Clean content for TTS by removing markdown and JSX syntax
     */
    private cleanForTTS(content: string): string {
        return content
            // Remove JSX/HTML tags
            .replace(/<[^>]*>/g, '')
            // Remove markdown formatting
            .replace(/\*\*([^*]+)\*\*/g, '$1') // Bold
            .replace(/\*([^*]+)\*/g, '$1')     // Italic
            .replace(/`([^`]+)`/g, '$1')       // Code
            .replace(/#{1,6}\s+/g, '')         // Headers
            .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1') // Links
            // Clean up whitespace
            .replace(/\s+/g, ' ')
            .trim();
    }
}

// Utility functions for integration with existing code

/**
 * Check if a chunk contains any section markers
 */
export function hasStructuredMarkers(text: string): boolean {
    return Object.values(MARKER_PATTERNS).some(pattern => pattern.test(text));
}

/**
 * Extract section type from the start of a chunk
 */
export function detectSectionType(chunk: string): 'markdown' | 'visual' | 'speech' | 'default' {
    if (MARKER_PATTERNS.MARKDOWN.test(chunk)) return 'markdown';
    if (MARKER_PATTERNS.VISUAL.test(chunk)) return 'visual';
    if (MARKER_PATTERNS.SPEECH.test(chunk)) return 'speech';
    return 'default';
}
