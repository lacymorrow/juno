import React from "react";
import DevToolsPanel from "@/components/DevToolsPanel";
import Header from "@/components/Header";
import ChatArea from "@/components/ChatArea";
import InputForm from "@/components/InputForm";
import LogsPanel from "@/components/LogsPanel";
import LogsSection from "@/components/LogsSection";
import { useServerStatus } from "@/hooks/useServerStatus";
import { useLogs } from "@/hooks/useLogs";
import { useAudio } from "@/hooks/useAudio";
import { useConversation } from "@/hooks/useConversation";

function App() {
  // First get the addLog function from useLogs
  const { 
    logs, 
    showLogs, 
    setShowLogs, 
    logsEndRef, 
    addLog, 
    getLogColorClass, 
    formatTimestamp,
    updateServerStatus
  } = useLogs("checking");
  
  // Then get the server status
  const { serverStatus, initialConversation } = useServerStatus(addLog);
  
  // Update the logs hook with the current server status
  React.useEffect(() => {
    updateServerStatus(serverStatus);
  }, [serverStatus, updateServerStatus]);
  const { playAudioFromBase64 } = useAudio(addLog);
  const { 
    query, 
    setQuery, 
    conversation, 
    isProcessing, 
    conversationEndRef, 
    handleSubmit 
  } = useConversation({
    initialConversation,
    serverStatus,
    addLog,
    playAudioFromBase64,
  });

  return (
    <div className="container mx-auto p-4 h-screen flex flex-col bg-background text-foreground">
      {/* Header */}
      <Header 
        serverStatus={serverStatus} 
        showLogs={showLogs} 
        setShowLogs={setShowLogs} 
      />

      {/* Main Content Area (Chat + Optional Logs) */}
      <div className="flex-grow flex overflow-hidden">
        {/* Chat Area */}
        <div className="flex-grow flex flex-col h-full pr-2">
          <ChatArea 
            conversation={conversation} 
            conversationEndRef={conversationEndRef} 
          />

          {/* Input Form */}
          <InputForm 
            query={query}
            setQuery={setQuery}
            handleSubmit={handleSubmit}
            isProcessing={isProcessing}
            serverStatus={serverStatus}
          />
        </div>

        {/* Logs Panel (Conditional) */}
        {showLogs && (
          <div className="flex-shrink-0 w-1/3 h-full pl-2 border-l">
            <LogsPanel 
              logs={logs}
              logsEndRef={logsEndRef}
              getLogColorClass={getLogColorClass}
              formatTimestamp={formatTimestamp}
            />
          </div>
        )}
      </div>

      {/* Developer Tools Panel */}
      <DevToolsPanel />

      {/* Logs Section */}
      <LogsSection 
        logs={logs}
        showLogs={showLogs}
        setShowLogs={setShowLogs}
        logsEndRef={logsEndRef}
        getLogColorClass={getLogColorClass}
        formatTimestamp={formatTimestamp}
      />
    </div>
  );
}

export default App;
