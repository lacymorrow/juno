// Type for conversation messages
export type ChatMessage = {
  role:
    | "user"
    | "assistant"
    | "system"
    | "thinking"
    | "tool_call_request"
    | "tool_call_result";
  content: string;
  isJsx?: boolean; // Flag to indicate if content should be rendered as JSX
  screenshot_base64?: string; // Optional base64 screenshot data
  tool_name?: string;
  tool_args?: any;
  tool_output?: any;
  success?: boolean; // For tool call results - indicates if the tool call was successful
  timestamp?: number; // Add timestamp field for message grouping
  isStreaming?: boolean; // Indicates if this message is currently being streamed
  messageId?: string; // Unique identifier for streaming messages
};

// Type for the result from submit_query
export type SubmitQueryResult = {
  text: string;
  spoken_text?: string; // Optional separate content for TTS speech
  audio_base64?: string; // Optional base64 audio data
  agent_state: string;
  screenshot_base64?: string; // Optional base64 screenshot data
};

// Type for the backend response event payload
export type BackendResponsePayload = {
  query: string;
  response: SubmitQueryResult;
};

// Streaming event types
export type StreamingTextEvent = {
  chunk: string;
  message_id?: string;
};

export type StreamStartEvent = {
  message_id: string;
};

export type StreamEndEvent = {
  message_id: string;
  complete_text: string;
};

// Agent Event Types (mirroring tool_logger.rs)
export interface ThinkingPayload {
  content: string;
}

export interface ToolCallRequestPayload {
  tool_name: string;
  tool_args: any; // Corresponds to serde_json::Value
  content?: string;
}

export interface ToolCallResultPayload {
  tool_name: string;
  tool_output: any; // Corresponds to serde_json::Value
  success: boolean;
  content?: string;
  screenshot_base64?: string;
}

export interface ScreenshotPayload {
  screenshot_base64: string;
  content?: string;
}

export interface GenericContentPayload {
  content: string;
}

export interface AgentEventTauri {
  type: string; // "thinking", "tool_call_request", "tool_call_result", "screenshot", "generic_content"
  payload: // This will be one of the specific payload types based on `type`
  | ThinkingPayload
    | ToolCallRequestPayload
    | ToolCallResultPayload
    | ScreenshotPayload
    | GenericContentPayload;
}

// Type for view state
export type AppView = "chat" | "devtools" | "permissions";

// Modal types for enhanced functionality
export type ModalType = "help" | "feedback" | "export" | "import" | "update" | null;

// Enhanced feedback form data
export interface FeedbackData {
  type: "issue" | "feature" | "general";
  title: string;
  description: string;
  email?: string;
  priority: "low" | "medium" | "high";
}

// Update check result
export interface UpdateInfo {
  available: boolean;
  version?: string;
  notes?: string;
  date?: string;
}

// Chat export format
export interface ChatExport {
  version: string;
  exported_at: string;
  conversation: ChatMessage[];
  metadata: {
    total_messages: number;
    export_type: "full" | "filtered";
  };
}

// Server status type
export type ServerStatus = "checking" | "connected" | "error";

// Keyboard shortcuts type
export interface KeyboardShortcuts {
  agent_mode_toggle: string;
  dictation_input: string;
  [key: string]: string;
}