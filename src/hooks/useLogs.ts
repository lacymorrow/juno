import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { LogEntry, ServerStatus } from "@/types";

export const useLogs = (initialStatus: ServerStatus) => {
  const [serverStatus, setServerStatus] = useState<ServerStatus>(initialStatus);
  
  // Function to update server status
  const updateServerStatus = useCallback((status: ServerStatus) => {
    setServerStatus(status);
  }, []);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [showLogs, setShowLogs] = useState(false);
  const logsEndRef = useRef<HTMLDivElement>(null);

  // Client-side log function
  const addLog = useCallback((message: string, level: string = "info") => {
    const newLog: LogEntry = { level, message, timestamp: Date.now() };
    setLogs((prev) => [...prev, newLog]);
    console.log(`[${level}] ${message}`);
  }, []);

  // Fetch logs periodically
  useEffect(() => {
    if (serverStatus !== "connected") return;

    const fetchBackendLogs = async () => {
      try {
        const backendLogs: string[] = await invoke("get_logs");

        if (backendLogs && backendLogs.length > 0) {
          const newLogEntries: LogEntry[] = backendLogs.map((logMsg) => {
            const match = logMsg.match(/^\[(.*?)\]\s*(.*)$/);
            if (match) {
              return {
                level: match[1],
                message: match[2],
                timestamp: Date.now(), // Assign timestamp on arrival
              };
            } else {
              return {
                level: "backend",
                message: logMsg,
                timestamp: Date.now(),
              };
            }
          });

          setLogs((prevLogs) => {
            const existingTimestamps = new Set(
              prevLogs.map((log) => log.timestamp)
            );
            const uniqueNewLogs = newLogEntries.filter(
              (newLog) => !existingTimestamps.has(newLog.timestamp)
            );
            return [...prevLogs, ...uniqueNewLogs].sort(
              (a, b) => a.timestamp - b.timestamp
            );
          });
        }
      } catch (error) {
        // Avoid logging the fetch error itself to prevent loops if get_logs fails
        console.error("Error fetching backend logs:", error);
      }
    };

    fetchBackendLogs();
    const interval = setInterval(fetchBackendLogs, 3000); // Poll every 3 seconds

    return () => clearInterval(interval);
  }, [serverStatus]);

  // Helper to get color class based on log level
  const getLogColorClass = (level: string): string => {
    switch (level.toLowerCase()) {
      case "error":
        return "text-red-500 dark:text-red-400";
      case "warn":
        return "text-yellow-500 dark:text-yellow-400";
      case "success":
        return "text-green-500 dark:text-green-400";
      case "backend":
      case "debug":
        return "text-purple-500 dark:text-purple-400";
      case "info":
      default:
        return "text-blue-500 dark:text-blue-400";
    }
  };

  // Helper to format timestamp
  const formatTimestamp = (timestamp: number): string => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12: false,
    });
  };

  return {
    logs,
    showLogs,
    setShowLogs,
    logsEndRef,
    addLog,
    getLogColorClass,
    formatTimestamp,
    updateServerStatus,
  };
};