import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ServerStatus, ChatMessage } from "@/types";

type UseServerStatusReturn = {
  serverStatus: ServerStatus;
  initialConversation: ChatMessage[];
};

export const useServerStatus = (addLog: (message: string, level?: string) => void): UseServerStatusReturn => {
  const [serverStatus, setServerStatus] = useState<ServerStatus>("checking");
  const [initialConversation, setInitialConversation] = useState<ChatMessage[]>([]);

  useEffect(() => {
    const checkServer = async () => {
      addLog("checking backend status...");
      try {
        const isConnected: boolean = await invoke("check_server_status");
        if (isConnected) {
          setServerStatus("connected");
          addLog("connected to backend", "success");
          setInitialConversation([
            {
              role: "system",
              content: "Connected. Enter your query below.",
            },
          ]);
        } else {
          setServerStatus("error");
          addLog("backend check failed (returned false)", "error");
          setInitialConversation([
            {
              role: "system",
              content: "Failed to connect to backend. Please check logs.",
            },
          ]);
        }
      } catch (error) {
        setServerStatus("error");
        addLog(`failed to invoke 'check_server_status': ${error}`, "error");
        setInitialConversation([
          {
            role: "system",
            content: `Error connecting to backend: ${error}. Check console logs.`,
          },
        ]);
      }
    };
    checkServer();
  }, [addLog]);

  return { serverStatus, initialConversation };
};