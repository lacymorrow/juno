import { UI } from "@/lib/constants.generated";

export type AssistantState =
	| typeof UI.AGENT_STATUS_IDLE
	| typeof UI.BAR_STATES_LISTENING
	| typeof UI.AGENT_STATUS_PROCESSING
	| typeof UI.BAR_STATES_SPEAKING
	| typeof UI.BAR_STATES_ERROR
	| typeof UI.BAR_STATES_SUCCESS
	| typeof UI.BAR_STATES_INPUT
	| typeof UI.AGENT_STATUS_RESPONDING;

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
