export type AssistantState =
	| "idle"
	| "listening"
	| "processing"
	| "speaking"
	| "error"
	| "success"
	| "input"
	| "response";

export type ContentType = "text" | "code" | "component" | "image" | "video";

export interface ResponseContent {
	type: ContentType;
	title: string;
	content: string;
}

export interface VoiceAIBarProps {
	onStateChange?: (state: AssistantState) => void;
	initialState?: AssistantState;
	className?: string;
	sampleResponses?: Record<string, ResponseContent>;
}
