import { UI } from "@/lib/constants.generated";

export type AssistantState =
	| typeof UI.AGENT_STATUS_IDLE
	| typeof UI.AGENT_STATUS_LISTENING
	| typeof UI.AGENT_STATUS_PROCESSING
	| typeof UI.AGENT_STATUS_RESPONDING
	| typeof UI.AGENT_STATUS_ERROR
	| typeof UI.AGENT_STATUS_FINISHED;

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
