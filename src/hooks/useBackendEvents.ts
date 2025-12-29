import { useEffect, useCallback, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { stopTTS } from "@/lib/ttsService";
import type { ChatMessage } from "@/components/ChatMessage";
import { EVENTS } from "@/lib/constants.generated";
import { safeCleanupEventListener } from "@/lib/safeEventCleanup";

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
	setUserHasScrolledUp: (scrolled: boolean) => void;

	// Auto-scroll function
	throttledAutoScroll: () => void;
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
	throttledAutoScroll,
}: UseBackendEventsProps) {
	const hasCheckedServer = useRef(false);

	// Handle backend responses via event listener
	const handleBackendResponse = useCallback(
		debounce((payload: BackendResponsePayload) => {
			console.log("Debounced handler executing for:", payload.query);
			const { response } = payload;

			// Check if we have any streaming assistant messages in progress or recently completed
			setConversationWithPruning((prevConversation) => {
				const hasStreamingMessage = prevConversation.some(
					(msg: ChatMessage) => msg.isStreaming && msg.role === "assistant"
				);

				// Check if this response matches a recently streamed message (to prevent duplicates)
				const now = Date.now();
				const isRecentlyStreamed = prevConversation.some(
					(msg: ChatMessage) =>
						msg.role === "assistant" &&
						msg.content === response.text &&
						msg.timestamp &&
						now - msg.timestamp < 2000 // Within last 2 seconds
				);

				// Only add assistant response message if we're not currently streaming
				if (!hasStreamingMessage && !isRecentlyStreamed) {
					console.log("Adding assistant message from backend response");
					const assistantMessage: ChatMessage = {
						role: "assistant",
						content: response.text,
						screenshot_base64: response.screenshot_base64,
						timestamp: Date.now(),
					};

					// Play audio if available (only when not streaming)
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

			// Reset processing state
			setIsProcessing(false);
		}, 100),
		[setConversationWithPruning, playAudioFromBase64, setIsProcessing]
	);

	// Server status check (with duplicate prevention for React Strict Mode)
	useEffect(() => {
		const checkServer = async () => {
			if (hasCheckedServer.current) return;
			hasCheckedServer.current = true;

			try {
				const isConnected: boolean = await invoke("check_server_status");
				if (isConnected) {
					setServerStatus("connected");
					addSystemMessage("Connected. Enter your query below.");
				} else {
					setServerStatus("error");
					addSystemMessage("Failed to connect to backend. Please check logs.");
				}
			} catch (error) {
				setServerStatus("error");
				addSystemMessage(`Error connecting to backend: ${error}. Check console logs.`);
			}
		};
		checkServer();
	}, [setServerStatus, addSystemMessage]);

	// Listen for responses broadcast from the backend
	useEffect(() => {
		let unlisten: (() => void) | undefined;

		const setupListener = async () => {
			unlisten = await listen<BackendResponsePayload>(
				EVENTS.SYSTEM_BACKEND_RESPONSE,
				(event) => {
					console.log("Received backend-response event (raw):", event.payload);
					handleBackendResponse(event.payload);
				}
			);
		};

		setupListener();
		return () => unlisten?.();
	}, [handleBackendResponse]);

	// Listen for system status updates for observability
	useEffect(() => {
		const unlisten = listen(EVENTS.SYSTEM_STATUS_UPDATE, (event) => {
			try {
				console.log("System status update:", event.payload);
			} catch (e) {
				console.warn("Failed to handle system status update:", e);
			}
		});

		return () => {
			unlisten.then((unlistenFn) => safeCleanupEventListener(unlistenFn));
		};
	}, []);

	// Listen for agent stopping events
	useEffect(() => {
		const unlisten = listen(EVENTS.AGENT_STOPPING, async () => {
			console.log("Agent stopping event received - stopping TTS");
			try {
				await stopTTS((msg, level) =>
					console.log(`[TTS-${level || "info"}] ${msg}`)
				);
			} catch (error) {
				console.error("Error stopping TTS:", error);
			}
		});

		return () => {
			unlisten.then((unlistenFn) => safeCleanupEventListener(unlistenFn));
		};
	}, []);

	// Listen for TTS audio ready events
	useEffect(() => {
		const unlisten = listen<{ audio_base64: string }>(
			EVENTS.TTS_AUDIO_READY,
			(event) => {
				console.log("TTS audio ready event received");
				const { audio_base64 } = event.payload;
				if (audio_base64) {
					playAudioFromBase64(audio_base64);
				}
			}
		);

		return () => {
			unlisten.then((unlistenFn) => safeCleanupEventListener(unlistenFn));
		};
	}, [playAudioFromBase64]);

	// Listen for TTS stop requests
	useEffect(() => {
		const unlisten = listen(EVENTS.TTS_STOP_REQUESTED, async () => {
			console.log("TTS stop requested event received - stopping TTS immediately");
			try {
				stopCurrentAudio();
				await stopTTS((msg, level) =>
					console.log(`[TTS-${level || "info"}] ${msg}`)
				);
			} catch (error) {
				console.error("Error stopping TTS:", error);
			}
		});

		return () => {
			unlisten.then((unlistenFn) => safeCleanupEventListener(unlistenFn));
		};
	}, [stopCurrentAudio]);

	// Listen for agent events (thinking, tool calls, etc.) - MERGED VERSION
	useEffect(() => {
		const unlistenPromise = listen<AgentEventTauri>(EVENTS.AGENT_EVENT, (event) => {
			const { type, payload } = event.payload;
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

			// Handle toast notifications with deduplication
			// Use unique IDs to prevent duplicate toasts for rapid events
			switch (type) {
				case "tool_call_request": {
					const notificationLevel = payload.notification_level || "standard";

					if (notificationLevel !== "silent") {
						const message = payload.content || `🔧 Executing ${payload.tool_name}...`;
						const duration = getNotificationDuration(notificationLevel, payload.estimated_duration);
						const toastId = `tool-request-${payload.tool_name}`;

						// Dismiss any previous request toast for the same tool
						toast.dismiss(toastId);
						toast.info(message, {
							id: toastId,
							duration,
							className: getNotificationClassName(payload.tool_category, "request"),
						});
					}
					break;
				}

				case "tool_call_result": {
					const notificationLevel = payload.notification_level || "standard";
					const success = payload.success ?? true;

					if (notificationLevel !== "silent") {
						const message = payload.content || (success ? `✅ Tool completed` : `❌ Tool failed`);
						const duration = getNotificationDuration(notificationLevel);
						const toastType = success ? "success" : "error";
						const toastId = `tool-result-${payload.tool_name}`;

						// Dismiss previous result and request toasts for this tool
						toast.dismiss(toastId);
						toast.dismiss(`tool-request-${payload.tool_name}`);
						toast[toastType](message, {
							id: toastId,
							duration,
							className: getNotificationClassName(payload.tool_category, "result", success),
						});

						if (payload.screenshot_base64 && success) {
							console.log("📸 Screenshot detected in tool result:", payload.tool_name);
							// Use consistent ID for screenshot toasts
							toast.dismiss("screenshot-captured");
							toast.success("📸 Screenshot captured", {
								id: "screenshot-captured",
								duration: 3000,
								className: "screenshot-notification",
							});
						}
					}
					break;
				}

				case "thinking": {
					if (payload.content) {
						// Use a single ID for thinking toasts - only show the latest
						toast.dismiss("thinking");
						toast.info(`💭 ${payload.content}`, {
							id: "thinking",
							duration: 2000,
							className: "thinking-notification",
						});
					}
					break;
				}

				case "screenshot": {
					// Use consistent ID for screenshot toasts
					toast.dismiss("screenshot-captured");
					toast.success("📸 Screenshot captured", {
						id: "screenshot-captured",
						duration: 3000,
						className: "screenshot-notification",
					});
					break;
				}

				case "generic_content": {
					if (payload.content) {
						// Use content-based ID for deduplication of generic content
						const contentHash = payload.content.slice(0, 50).replace(/\s+/g, '-');
						const toastId = `generic-${contentHash}`;
						toast.dismiss(toastId);
						toast.info(payload.content, {
							id: toastId,
							duration: 4000,
							className: "generic-content-notification",
						});
					}
					break;
				}

				default:
					// Handle any other event types silently
					break;
			}

			// Auto-scroll to show new messages
			if (type === "tool_call_result" || type === "tool_call_request" || type === "thinking") {
				throttledAutoScroll();
			}
		});

		return () => {
			unlistenPromise.then((unlistenFn) => unlistenFn());
		};
	}, [setConversationWithPruning, throttledAutoScroll]);

	// Listen for streaming events
	useEffect(() => {
		const streamStartListener = listen<StreamStartEvent>(
			EVENTS.STREAMING_STREAM_START,
			(event) => {
				console.log("Stream started:", event.payload);
				const { message_id } = event.payload;

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

		const streamTextListener = listen<StreamingTextEvent>(
			EVENTS.STREAMING_TEXT_STREAM,
			(event) => {
				console.log("Stream text chunk:", event.payload);
				const { chunk, message_id, tts_content } = event.payload;

				setConversationWithPruning((prev) =>
					prev.map((msg) => {
						if (msg.messageId === message_id && msg.isStreaming) {
							// Collect TTS content for decorative display
							const existingTtsContent = msg.tts_metadata?.tts_parts || [];
							const newTtsContent = tts_content ? [...existingTtsContent, tts_content] : existingTtsContent;

							return {
								...msg,
								content: msg.content + chunk,
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

				throttledAutoScroll();
			}
		);

		const streamEndListener = listen<StreamEndEvent>(
			EVENTS.STREAMING_STREAM_END,
			(event) => {
				console.log("Stream ended:", event.payload);
				const { message_id, complete_text, agent_state } = event.payload;

				setConversationWithPruning((prev) =>
					prev.map((msg) => {
						if (msg.messageId === message_id && msg.isStreaming) {
							return {
								...msg,
								content: complete_text,
								isStreaming: false,
								agent_state,
							};
						}
						return msg;
					})
				);

				setIsProcessing(false);
			}
		);

		return () => {
			streamStartListener.then((unlistenFn) => unlistenFn());
			streamTextListener.then((unlistenFn) => unlistenFn());
			streamEndListener.then((unlistenFn) => unlistenFn());
		};
	}, [setConversationWithPruning, throttledAutoScroll, setIsProcessing]);

	// Listen for thinking streaming events (streamed thinking content)
	useEffect(() => {
		// Thinking stream start - create a new streaming thinking message
		const thinkingStartListener = listen<{ message_id: string }>(
			EVENTS.STREAMING_THINKING_START,
			(event) => {
				console.log("Thinking stream started:", event.payload);
				const { message_id } = event.payload;

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

		// Thinking stream chunk - append to the streaming thinking message
		const thinkingStreamListener = listen<{ chunk: string; message_id: string | null }>(
			EVENTS.STREAMING_THINKING_STREAM,
			(event) => {
				const { chunk, message_id } = event.payload;

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

				throttledAutoScroll();
			}
		);

		// Thinking stream end - finalize the streaming thinking message
		const thinkingEndListener = listen<{ message_id: string; complete_text: string }>(
			EVENTS.STREAMING_THINKING_END,
			(event) => {
				console.log("Thinking stream ended:", event.payload);
				const { message_id, complete_text } = event.payload;

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

		return () => {
			thinkingStartListener.then((unlistenFn) => unlistenFn());
			thinkingStreamListener.then((unlistenFn) => unlistenFn());
			thinkingEndListener.then((unlistenFn) => unlistenFn());
		};
	}, [setConversationWithPruning, throttledAutoScroll]);

	// Listen for agent error events
	useEffect(() => {
		const unlisten = listen<{
			agent_state: string;
			error_message: string;
			original_query: string;
		}>(EVENTS.AGENT_ERROR, (event) => {
			console.log("Agent error event received:", event.payload);
			const { agent_state, error_message } = event.payload;

			setIsProcessing(false);
			addSystemMessage(`Agent ${agent_state.toLowerCase()}: ${error_message}`);
		});

		return () => {
			unlisten.then((unlistenFn) => safeCleanupEventListener(unlistenFn));
		};
	}, [setIsProcessing, addSystemMessage]);

	// Listen for agent continuation requests
	useEffect(() => {
		const unlisten = listen<{
			request_id: string;
			execution_id: string;
			current_step: number;
			max_steps: number;
			message: string;
		}>(EVENTS.CONTINUATION_AGENT_REQUEST, (event) => {
			console.log("Agent continuation request received:", event.payload);
			const { request_id, current_step, max_steps, message } = event.payload;

			// Add system message to conversation
			addSystemMessage(
				`🔄 Agent reached ${max_steps} step limit (step ${current_step}). Requesting continuation...`
			);

			// Show primary action toast with stop as the prominent action
			toast.error(`⏹️ ${message}`, {
				duration: 300000, // 5 minutes to match backend timeout
				id: `continuation-${request_id}`,
				description: `Step ${current_step}/${max_steps} - Agent has reached iteration limit`,
				action: {
					label: "🛑 Stop Agent",
					onClick: () => {
						invoke("respond_to_agent_continuation", {
							requestId: request_id,
							approved: false
						}).then(() => {
							toast.dismiss(`continuation-${request_id}`);
							toast.dismiss(`continuation-continue-${request_id}`);
							toast.success("✅ Agent execution stopped", {
								id: `continuation-denied-${request_id}`,
								duration: 3000,
							});
						}).catch((error) => {
							console.error("Failed to deny continuation:", error);
							toast.error("Failed to stop agent", {
								duration: 5000,
							});
						});
					},
				},
				closeButton: false, // Don't allow dismissing without action
				className: "agent-continuation-toast-stop",
			});

			// Show secondary toast for continuation option
			setTimeout(() => {
				toast.warning("⚠️ Or click here to continue (not recommended)", {
					duration: 300000, // Same timeout
					id: `continuation-continue-${request_id}`,
					description: "This will add 20 more steps and may continue indefinitely",
					action: {
						label: "▶️ Continue (+20 steps)",
						onClick: () => {
							invoke("respond_to_agent_continuation", {
								requestId: request_id,
								approved: true,
								additionalSteps: 20
							}).then(() => {
								toast.dismiss(`continuation-${request_id}`);
								toast.dismiss(`continuation-continue-${request_id}`);
								toast.info("Agent continuation approved (+20 steps)", {
									id: `continuation-approved-${request_id}`,
									duration: 3000,
								});
							}).catch((error) => {
								console.error("Failed to approve continuation:", error);
								toast.error("Failed to approve continuation", {
									duration: 5000,
								});
							});
						},
					},
					closeButton: false, // Don't allow dismissing without action
					className: "agent-continuation-toast-continue",
				});
			}, 100); // Small delay to show both toasts
		});

		return () => {
			unlisten.then((unlistenFn) => safeCleanupEventListener(unlistenFn));
		};
	}, [addSystemMessage]);

	// Listen for agent continuation responses
	useEffect(() => {
		const unlisten = listen<{
			request_id: string;
			approved: boolean;
			additional_steps?: number;
		}>(EVENTS.CONTINUATION_AGENT_RESPONSE, (event) => {
			console.log("Agent continuation response received:", event.payload);
			const { approved, additional_steps } = event.payload;

			if (approved) {
				const steps = additional_steps || 20;
				addSystemMessage(
					`✅ Agent continuation approved (+${steps} steps). Resuming execution...`
				);
			} else {
				addSystemMessage("❌ Agent continuation denied. Execution stopped.");
			}
		});

		return () => {
			unlisten.then((unlistenFn) => safeCleanupEventListener(unlistenFn));
		};
	}, [addSystemMessage]);

	// Listen for comprehensive agent-stop-all events
	useEffect(() => {
		const unlisten = listen(EVENTS.AGENT_STOP_ALL, async () => {
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
		});

		return () => {
			unlisten.then((unlistenFn) => safeCleanupEventListener(unlistenFn));
		};
	}, [setIsProcessing, stopCurrentAudio]);

	// Listen for dictation state changes (for synchronization/debugging)
	useEffect(() => {
		const unlisten = listen<DictationStateChangeEvent>(
			EVENTS.DICTATION_STATE_CHANGED,
			(event) => {
				console.log("Dictation state changed:", event.payload);
				// Note: Primary UI state is driven by useAppState/voice_mode, 
				// but this event provides detailed transition info for debugging
				// or future fine-grained UI updates.
			}
		);

		return () => {
			unlisten.then((unlistenFn) => safeCleanupEventListener(unlistenFn));
		};
	}, []);

	// Listen for user message submitted events (from voice input)
	useEffect(() => {
		const unlisten = listen<{ content: string; timestamp: number }>(
			EVENTS.MESSAGES_USER_MESSAGE_SUBMITTED,
			(event) => {
				console.log("User message submitted event received:", event.payload);
				const { content, timestamp } = event.payload;

				setConversationWithPruning((prev) => [
					...prev,
					{
						role: "user",
						content,
						timestamp,
					}
				]);
			}
		);

		return () => {
			unlisten.then((unlistenFn) => safeCleanupEventListener(unlistenFn));
		};
	}, [setConversationWithPruning]);
}

// Helper functions for notifications
function getNotificationDuration(notificationLevel: string, estimatedDuration?: string): number {
	const baseDurations = {
		minimal: 1500,
		standard: 3000,
		detailed: 5000,
	};

	const baseDuration = baseDurations[notificationLevel as keyof typeof baseDurations] || 3000;

	if (estimatedDuration) {
		const durationMultipliers = {
			instant: 0.5,
			short: 0.8,
			medium: 1.0,
			long: 1.5,
		};
		const multiplier = durationMultipliers[estimatedDuration as keyof typeof durationMultipliers] || 1.0;
		return Math.round(baseDuration * multiplier);
	}

	return baseDuration;
}

function getNotificationClassName(
	toolCategory?: string,
	eventType?: string,
	success?: boolean
): string {
	let className = "tool-notification";

	if (toolCategory) {
		className += ` ${toolCategory.toLowerCase()}-category`;
	}

	if (eventType) {
		className += ` ${eventType}-event`;
	}

	if (eventType === "result" && success !== undefined) {
		className += success ? " success-result" : " failure-result";
	}

	return className;
}
