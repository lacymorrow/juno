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
	// Optional external input handling props for integration with FloatingBar
	inputValue?: string;
	onInputChange?: (value: string) => void;
	onInputSubmit?: (e: React.FormEvent) => void;
	onInputBlur?: () => void;
	inputRef?: React.RefObject<HTMLInputElement>;
}

export interface DevPanelProps {
	currentState: AssistantState;
	onStateChange: (state: AssistantState) => void;
	sampleResponses?: Record<string, ResponseContent>;
	isVisible: boolean;
	onToggleVisibility: () => void;
}
