import { toast } from 'sonner';

export interface ErrorWithMessage {
  message: string;
}

/**
 * Formats agent/provider errors for chat UI display.
 *
 * Goal: never show raw JSON blobs (e.g. Anthropic error payloads) to the user.
 * If a structured error body is present, we extract the most human-readable message.
 */
export function formatAgentChatErrorText(raw: string): string {
  const input = String(raw ?? '').trim();
  if (!input) return '';

  // Fast-path: strip request_id suffix if present (common in provider error strings).
  const withoutRequestId = input.replace(/\s*\(request_id:\s*[^)]+\)\s*$/i, '').trim();

  // If the string ends with an embedded JSON object, try to parse and extract `.error.message` or `.message`.
  const lastBraceIndex = withoutRequestId.lastIndexOf('{');
  if (lastBraceIndex !== -1 && withoutRequestId.endsWith('}')) {
    const jsonCandidate = withoutRequestId.slice(lastBraceIndex).trim();

    // Avoid parsing arbitrary braces; only attempt if it looks like a structured error.
    if (jsonCandidate.includes('"message"') && (jsonCandidate.includes('"error"') || jsonCandidate.includes('"type"'))) {
      try {
        const parsed = JSON.parse(jsonCandidate) as any;
        const extractedMessage =
          parsed?.error?.message ??
          parsed?.message ??
          parsed?.error?.error?.message;

        if (typeof extractedMessage === 'string' && extractedMessage.trim()) {
          return extractedMessage.trim();
        }
      } catch {
        // Fall through to regex extraction.
        const messageMatch = jsonCandidate.match(/"message"\s*:\s*"([^"]+)"/);
        if (messageMatch?.[1]) {
          return messageMatch[1].trim();
        }
      }
    }
  }

  // For provider-style messages like:
  // "Anthropic API error 400 Bad Request (invalid_request_error): <message>"
  // return only the tail message.
  if (/anthropic api (error|returned error)/i.test(withoutRequestId)) {
    const lastColonSpace = withoutRequestId.lastIndexOf(': ');
    if (lastColonSpace !== -1) {
      const tail = withoutRequestId.slice(lastColonSpace + 2).trim();
      if (tail) return tail;
    }
  }

  return withoutRequestId;
}

export function isErrorWithMessage(error: unknown): error is ErrorWithMessage {
  return (
    typeof error === 'object' &&
    error !== null &&
    'message' in error &&
    typeof (error as Record<string, unknown>).message === 'string'
  );
}

export function toErrorWithMessage(maybeError: unknown): ErrorWithMessage {
  if (isErrorWithMessage(maybeError)) return maybeError;

  try {
    return new Error(JSON.stringify(maybeError));
  } catch {
    // fallback in case there's an error stringifying the maybeError
    // like with circular references for example.
    return new Error(String(maybeError));
  }
}

export function handleError(error: unknown, context?: string): void {
  const errorWithMessage = toErrorWithMessage(error);
  const message = context 
    ? `${context}: ${errorWithMessage.message}` 
    : errorWithMessage.message;
  
  console.error(message, error);
  toast.error(message);
}

export async function withErrorHandling<T>(
  operation: () => Promise<T>,
  context?: string
): Promise<T | null> {
  try {
    return await operation();
  } catch (error) {
    handleError(error, context);
    return null;
  }
}