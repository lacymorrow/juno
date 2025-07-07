import { UI } from "@/lib/constants.generated";

export type AssistantState =
	| typeof UI.AGENT_STATUS_IDLE
	| typeof UI.AGENT_STATUS_DICTATING
	| typeof UI.AGENT_STATUS_LISTENING
	| typeof UI.AGENT_STATUS_THINKING
	| typeof UI.AGENT_STATUS_RESPONDING
	| typeof UI.AGENT_STATUS_ERROR
	| typeof UI.AGENT_STATUS_FINISHED
	| typeof UI.AGENT_STATUS_FAILED
	| typeof UI.AGENT_STATUS_CANCELLED
	| typeof UI.AGENT_STATUS_OFFLINE
	| typeof UI.AGENT_STATUS_PROCESSING
	| "speaking"
	| "input"
	| "success"
	| "response";

export type ContentType = "text" | "code" | "component" | "image" | "video"

export interface ResponseContent {
	type: ContentType
	content: string
	title?: string
}

export interface VoiceAIBarProps {
	onStateChange?: (state: AssistantState) => void
	initialState?: AssistantState
	className?: string
	sampleResponses?: Record<string, ResponseContent>
}

export interface DevPanelProps {
	currentState: AssistantState
	onStateChange: (state: AssistantState) => void
	sampleResponses: Record<string, ResponseContent>
	isVisible: boolean
	onToggleVisibility: () => void
}
