import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { stopTTS } from "@/lib/ttsService";
import type { ChatMessage } from "@/types/chat";
import { EVENTS } from "@/lib/constants.generated";
import { useEventListener } from "@/hooks/useEventListener";
import { hasMixedContent } from "@/components/ui/mixed-content-renderer";

// AX grounding audit payload emitted before each computer use click
type AxGroundingAuditEvent = {
	action: string;
	ax_grounded: boolean;
	ax_role: string | null;
	ax_label: string | null;
	screen_coordinate: [number, number];
	timestamp: number;
};

// Type definitions for backend events
type SubmitQueryResult = {
	text: string;
	spoken_text?: string;
	audio_base64?: string;
	agent_state: string;
	screenshot_base64?: string;
};

type BackendResponsePayload = {
	query: string;
	response: SubmitQueryResult;
};

type ServerStatus = {
	backend_running: boolean;
	desktop_available: boolean;
};

type StreamingTextEvent = {
	chunk: string;
	message_id?: string;
	tts_content?: string;
	metadata?: {
		has_spoken_content?: boolean;
		spoken_text?: string;
	};
};

type StreamStartEvent = {
	message_id: string;
};

type StreamEndEvent = {
	message_id: string;
	complete_text: string;
	agent_state?: string; // "Finished", "Failed", "Cancelled", "Offline"
	is_jsx?: boolean; // true if content contains JSX components to render
};

type DictationStateChangeEvent = {
	previous_state: string;
	new_state: string;
	timestamp: number;
	reason: string;
	component: string;
};

interface AgentEventTauri {
	type: string;
	payload: {
		content?: string;
		tool_name?: string;
		tool_args?: any;
		tool_output?: any;
		success?: boolean;
		screenshot_base64?: string;
		tool_category?: string;
		notification_level?: string;
		estimated_duration?: string;
		[key: string]: any;
	};
}

interface UseBackendEventsProps {
	// Conversation management
	addSystemMessage: (content: string) => void;
	addAssistantMessage: (content: string, metadata?: Partial<ChatMessage>) => void;
	setConversationWithPruning: (updateFn: React.SetStateAction<ChatMessage[]>) => void;

	// Audio management
	playAudioFromBase64: (base64Audio: string) => void;
	stopCurrentAudio: () => void;

	// State management
	setIsProcessing: (processing: boolean) => void;
	setServerStatus: (status: "connected" | "error" | "connecting") => void;

	// A secondary window (the floating bar) mirrors the conversation but must
	// not announce "Connected…" or probe the backend: the main window owns that.
	skipServerCheck?: boolean;
}

// Simple debounce function
function debounce<F extends (...args: any[]) => any>(func: F, waitFor: number) {
	let timeoutId: ReturnType<typeof setTimeout> | null = null;

	return (...args: Parameters<F>): void => {
		if (timeoutId !== null) {
			clearTimeout(timeoutId);
		}
		timeoutId = setTimeout(() => func(...args), waitFor);
	};
}

export function useBackendEvents({
	addSystemMessage,
	setConversationWithPruning,
	playAudioFromBase64,
	stopCurrentAudio,
	setIsProcessing,
	setServerStatus,
	skipServerCheck = false,
}: UseBackendEventsProps) {
	const hasCheckedServer = useRef(false);
	// Single-slot buffer: ax-grounding-audit fires before tool_call_result, so we
	// store the latest unattached audit here and consume it when the result arrives.
	const lastAxAudit = useRef<AxGroundingAuditEvent | null>(null);

	// Handle backend responses via event listener.
	// Use refs to keep the debounced function stable while always calling latest handler.
	const backendResponseHandlerRef = useRef<(payload: BackendResponsePayload) => void>(() => {});
	backendResponseHandlerRef.current = (payload: BackendResponsePayload) => {
		console.log("Debounced handler executing for:", payload.query);
		const { response } = payload;

		setConversationWithPruning((prevConversation) => {
			const hasStreamingMessage = prevConversation.some(
				(msg: ChatMessage) => msg.isStreaming && msg.role === "assistant"
			);

			const now = Date.now();
			const isRecentlyStreamed = prevConversation.some(
				(msg: ChatMessage) =>
					msg.role === "assistant" &&
					msg.content === response.text &&
					msg.timestamp &&
					now - msg.timestamp < 2000
			);

			if (!hasStreamingMessage && !isRecentlyStreamed) {
				console.log("Adding assistant message from backend response");
				const assistantMessage: ChatMessage = {
					role: "assistant",
					content: response.text,
					screenshot_base64: response.screenshot_base64,
					timestamp: Date.now(),
				};

				if (response.audio_base64) {
					playAudioFromBase64(response.audio_base64);
				}

				return [...prevConversation, assistantMessage];
			} else {
				if (hasStreamingMessage) {
					console.log("Skipping assistant message addition - streaming in progress");
				} else if (isRecentlyStreamed) {
					console.log("Skipping assistant message addition - recently streamed duplicate");
				}
				return prevConversation;
			}
		});

		setIsProcessing(false);
	};

	// Stable debounced function — created once, always calls latest handler via ref
	const handleBackendResponse = useRef(
		debounce((payload: BackendResponsePayload) => {
			backendResponseHandlerRef.current(payload);
		}, 100)
	).current;

	// Server status check (with duplicate prevention for React Strict Mode)
	useEffect(() => {
		if (skipServerCheck) return;
		const checkServer = async () => {
			if (hasCheckedServer.current) return;
			hasCheckedServer.current = true;

			try {
				const status: ServerStatus = await invoke("check_server_status");
				if (status.backend_running) {
					setServerStatus("connected");
					if (status.desktop_available) {
						addSystemMessage("Connected. Enter your query below.");
					} else {
						addSystemMessage("Connected. Desktop automation requires accessibility permissions — grant them in System Settings > Privacy & Security > Accessibility.");
					}
				} else {
					setServerStatus("error");
					addSystemMessage("Backend is not responding. Please check logs.");
				}
			} catch (error) {
				setServerStatus("error");
				addSystemMessage(`Error connecting to backend: ${error}. Check console logs.`);
			}
		};
		checkServer();
	}, [setServerStatus, addSystemMessage, skipServerCheck]);

	// Listen for responses broadcast from the backend
	useEventListener<BackendResponsePayload>(
		EVENTS.SYSTEM_BACKEND_RESPONSE,
		(payload) => {
			console.log("Received backend-response event (raw):", payload);
			handleBackendResponse(payload);
		}
	);

	// Listen for system status updates for observability
	useEventListener(
		EVENTS.SYSTEM_STATUS_UPDATE,
		(payload) => {
			console.log("System status update:", payload);
		}
	);

	// Listen for agent stopping events
	useEventListener(
		EVENTS.AGENT_STOPPING,
		async () => {
			console.log("Agent stopping event received - stopping TTS");
			try {
				await stopTTS((msg, level) =>
					console.log(`[TTS-${level || "info"}] ${msg}`)
				);
			} catch (error) {
				console.error("Error stopping TTS:", error);
			}
		}
	);

	// Listen for TTS audio ready events
	useEventListener<{ audio_base64: string }>(
		EVENTS.TTS_AUDIO_READY,
		(payload) => {
			console.log("TTS audio ready event received");
			if (payload.audio_base64) {
				playAudioFromBase64(payload.audio_base64);
			}
		}
	);

	// Listen for TTS stop requests
	useEventListener(
		EVENTS.TTS_STOP_REQUESTED,
		async () => {
			console.log("TTS stop requested event received - stopping TTS immediately");
			try {
				stopCurrentAudio();
				await stopTTS((msg, level) =>
					console.log(`[TTS-${level || "info"}] ${msg}`)
				);
			} catch (error) {
				console.error("Error stopping TTS:", error);
			}
		}
	);

	// Listen for agent events (thinking, tool calls, etc.)
	useEventListener<AgentEventTauri>(
		EVENTS.AGENT_EVENT,
		(agentEvent) => {
			const { type, payload } = agentEvent;
			const currentTime = Date.now();

			// Handle conversation messages
			setConversationWithPruning((prev) => {
				let newMessage: ChatMessage | null = null;

				if (type === "thinking" && payload.content) {
					newMessage = {
						role: "thinking",
						content: payload.content || "Thinking...",
						timestamp: currentTime,
					};
				} else if (type === "tool_call_request" && payload.tool_name) {
					newMessage = {
						role: "tool_call_request",
						tool_name: payload.tool_name,
						tool_args: payload.tool_args,
						content: payload.content || `Using tool: ${payload.tool_name}`,
						timestamp: currentTime,
					};
				} else if (type === "tool_call_result" && payload.tool_name) {
					// Consume the AX audit buffer for computer use tool results
					const axAudit = payload.tool_name === "computer" ? lastAxAudit.current : null;
					lastAxAudit.current = null;

					newMessage = {
						role: "tool_call_result",
						tool_name: payload.tool_name,
						tool_output: payload.tool_output,
						success: payload.success,
						content: payload.content ||
							(payload.success
								? `Tool ${payload.tool_name} executed successfully.`
								: `Tool ${payload.tool_name} failed.`),
						screenshot_base64: payload.screenshot_base64,
						timestamp: currentTime,
						...(axAudit && {
							ax_grounded: axAudit.ax_grounded,
							ax_role: axAudit.ax_role,
							ax_label: axAudit.ax_label,
							ax_screen_coordinate: axAudit.screen_coordinate,
							ax_action: axAudit.action,
						}),
					};
				} else if (type === "generic_content" && payload.content) {
					newMessage = {
						role: "system",
						content: payload.content || "System message",
						timestamp: currentTime,
					};
				}

				if (newMessage) {
					return [...prev, newMessage];
				}
				return prev;
			});
		}
	);

	// Listen for tool-approval-request — add inline approval message to conversation
	useEventListener<{
		tool_name: string;
		tool_id: string;
		tool_input: any;
		description: string;
		timestamp: number;
		risk_level?: "low" | "medium" | "high" | "critical";
		target_app?: string;
		timeout_seconds?: number;
	}>(
		"tool-approval-request",
		(payload) => {
			console.log("Tool approval request received (inline):", payload);
			setConversationWithPruning((prev) => [
				...prev,
				{
					role: "tool_call_request",
					content: payload.description,
					tool_name: payload.tool_name,
					tool_args: payload.tool_input,
					tool_id: payload.tool_id,
					approval_state: "pending",
					risk_level: payload.risk_level,
					target_app: payload.target_app,
					approval_timeout_seconds: payload.timeout_seconds ?? 60,
					timestamp: payload.timestamp,
				},
			]);
		}
	);

	// Buffer AX grounding audit events — consumed by the next tool_call_result for "computer" tool
	useEventListener<AxGroundingAuditEvent>(
		EVENTS.TOOLS_AX_GROUNDING_AUDIT,
		(payload) => {
			lastAxAudit.current = payload;
		}
	);

	// Listen for streaming start events
	useEventListener<StreamStartEvent>(
		EVENTS.STREAMING_STREAM_START,
		(payload) => {
			console.log("Stream started:", payload);
			const { message_id } = payload;

			const streamingMessage: ChatMessage = {
				role: "assistant",
				content: "",
				timestamp: Date.now(),
				isStreaming: true,
				messageId: message_id,
			};

			setConversationWithPruning((prev) => [...prev, streamingMessage]);
		}
	);

	// Listen for streaming text chunks
	useEventListener<StreamingTextEvent>(
		EVENTS.STREAMING_TEXT_STREAM,
		(payload) => {
			const { chunk, message_id, tts_content } = payload;

			setConversationWithPruning((prev) =>
				prev.map((msg) => {
					if (msg.messageId === message_id && msg.isStreaming) {
						const existingTtsContent = msg.tts_metadata?.tts_parts || [];
						const newTtsContent = tts_content ? [...existingTtsContent, tts_content] : existingTtsContent;
						const updatedContent = msg.content + chunk;

						// Detect JSX during streaming so MixedContentRenderer kicks in immediately
						// Once detected, stays true for the rest of the stream
						const isJsx = msg.isJsx || hasMixedContent(updatedContent);

						return {
							...msg,
							content: updatedContent,
							isJsx,
							tts_metadata: {
								has_spoken_content: (msg.tts_metadata?.has_spoken_content || false) || !!tts_content,
								tts_parts: newTtsContent,
								total_spoken_text: newTtsContent.join(' ')
							}
						};
					}
					return msg;
				})
			);
		}
	);

	// Listen for streaming end events
	useEventListener<StreamEndEvent>(
		EVENTS.STREAMING_STREAM_END,
		(payload) => {
			console.log("Stream ended:", payload);
			const { message_id, complete_text, agent_state, is_jsx } = payload;

			setConversationWithPruning((prev) =>
				prev.map((msg) => {
					if (msg.messageId === message_id && msg.isStreaming) {
						return {
							...msg,
							content: complete_text,
							isStreaming: false,
							agent_state,
							// Preserve frontend JSX detection — backend can upgrade to true
						// but never downgrade (prevents flash if backend payload omits is_jsx)
						isJsx: is_jsx || msg.isJsx || false,
						};
					}
					return msg;
				})
			);

			setIsProcessing(false);
		}
	);

	// Listen for thinking stream start
	useEventListener<{ message_id: string }>(
		EVENTS.STREAMING_THINKING_START,
		(payload) => {
			console.log("Thinking stream started:", payload);
			const { message_id } = payload;

			const thinkingMessage: ChatMessage = {
				role: "thinking",
				content: "",
				messageId: message_id,
				isStreaming: true,
				timestamp: Date.now(),
			};

			setConversationWithPruning((prev) => [...prev, thinkingMessage]);
		}
	);

	// Listen for thinking stream chunks
	useEventListener<{ chunk: string; message_id: string | null }>(
		EVENTS.STREAMING_THINKING_STREAM,
		(payload) => {
			const { chunk, message_id } = payload;

			setConversationWithPruning((prev) =>
				prev.map((msg) => {
					if (msg.messageId === message_id && msg.isStreaming && msg.role === "thinking") {
						return {
							...msg,
							content: msg.content + chunk,
						};
					}
					return msg;
				})
			);
		}
	);

	// Listen for thinking stream end
	useEventListener<{ message_id: string; complete_text: string }>(
		EVENTS.STREAMING_THINKING_END,
		(payload) => {
			console.log("Thinking stream ended:", payload);
			const { message_id, complete_text } = payload;

			setConversationWithPruning((prev) =>
				prev.map((msg) => {
					if (msg.messageId === message_id && msg.isStreaming && msg.role === "thinking") {
						return {
							...msg,
							content: complete_text,
							isStreaming: false,
						};
					}
					return msg;
				})
			);
		}
	);

	// Listen for agent error events
	useEventListener<{
		agent_state: string;
		error_message: string;
		original_query: string;
	}>(
		EVENTS.AGENT_ERROR,
		(payload) => {
			console.log("Agent error event received:", payload);
			const { agent_state, error_message } = payload;

			setIsProcessing(false);
			addSystemMessage(`Agent ${agent_state.toLowerCase()}: ${error_message}`);
		}
	);

	// Listen for agent continuation requests — inline chat message with action buttons
	useEventListener<{
		request_id: string;
		execution_id: string;
		current_step: number;
		max_steps: number;
		message: string;
	}>(
		EVENTS.CONTINUATION_AGENT_REQUEST,
		(payload) => {
			console.log("Agent continuation request received:", payload);
			const { request_id, current_step, max_steps } = payload;

			setConversationWithPruning((prev) => [
				...prev,
				{
					role: "system",
					content: `Agent reached step ${current_step}/${max_steps}. Stop or continue?`,
					continuation_request_id: request_id,
					continuation_state: "pending",
					timestamp: Date.now(),
				},
			]);
		}
	);

	// Listen for agent continuation responses
	useEventListener<{
		request_id: string;
		approved: boolean;
		additional_steps?: number;
	}>(
		EVENTS.CONTINUATION_AGENT_RESPONSE,
		(payload) => {
			console.log("Agent continuation response received:", payload);
			const { approved, additional_steps } = payload;

			if (approved) {
				const steps = additional_steps || 20;
				addSystemMessage(
					`✅ Agent continuation approved (+${steps} steps). Resuming execution...`
				);
			} else {
				addSystemMessage("❌ Agent continuation denied. Execution stopped.");
			}
		}
	);

	// Listen for comprehensive agent-stop-all events
	useEventListener(
		EVENTS.AGENT_STOP_ALL,
		async () => {
			console.log("Agent stop all event received - performing comprehensive UI cleanup");
			try {
				await stopTTS((msg, level) =>
					console.log(`[Agent Stop All TTS-${level || "info"}] ${msg}`)
				);
				setIsProcessing(false);
				stopCurrentAudio();
				console.log("Agent stop all: UI cleanup completed successfully");
			} catch (error) {
				console.error("Error during agent stop all cleanup:", error);
			}
		}
	);

	// Listen for dictation state changes (for synchronization/debugging)
	useEventListener<DictationStateChangeEvent>(
		EVENTS.DICTATION_STATE_CHANGED,
		(payload) => {
			console.log("Dictation state changed:", payload);
		}
	);

	// The backend announces every accepted query here, whatever its source
	// (typed, voice, bar, rendered component, cloud, scheduled). This is the
	// only place a user message enters the conversation and the only place
	// processing switches on, so every source looks identical to the user.
	useEventListener<{ content: string; timestamp: number }>(
		EVENTS.MESSAGES_USER_MESSAGE_SUBMITTED,
		(payload) => {
			console.log("User message submitted event received:", payload);
			const { content, timestamp } = payload;

			setConversationWithPruning((prev) => [
				...prev,
				{
					role: "user",
					content,
					timestamp,
				}
			]);
			setIsProcessing(true);
		}
	);

	// Backend clears agent activity on every failure path that never reaches
	// a stream end or backend response (e.g. a submission rejected before the
	// agent runs), so processing can never get stuck on.
	useEventListener<boolean>(EVENTS.AGENT_ACTIVE, (active) => {
		if (!active) {
			setIsProcessing(false);
		}
	});
}

