// Chat message types
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

// Backend response types
export type SubmitQueryResult = {
  text: string;
  audio_base64?: string; // Optional base64 audio data
  agent_state: string;
  screenshot_base64?: string; // Optional base64 screenshot data
};

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

// Agent event payload types
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

// App view types
export type AppView = "chat" | "settings" | "devtools" | "permissions" | "onboarding";

// Modal types
export type ModalType = "help" | "feedback" | "export" | "import" | "update" | null;

export interface FeedbackData {
  type: "issue" | "feature" | "general";
  title: string;
  description: string;
  email?: string;
  priority: "low" | "medium" | "high";
}

export interface UpdateInfo {
  available: boolean;
  version?: string;
  notes?: string;
  date?: string;
}

export interface ChatExport {
  version: string;
  exported_at: string;
  conversation: ChatMessage[];
  metadata: {
    total_messages: number;
    export_type: "full" | "filtered";
  };
}