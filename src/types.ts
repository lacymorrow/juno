// Type for conversation messages
export type ChatMessage = {
  role: "user" | "assistant" | "system";
  content: string;
};

// Type for the result from submit_query
export type SubmitQueryResult = {
  text: string;
  audio_base64?: string; // Optional base64 audio data
};

// Type for logs
export type LogEntry = {
  level: string;
  message: string;
  timestamp: number;
};

// Server status type
export type ServerStatus = "checking" | "connected" | "error";