import React from "react";
import { Send } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ServerStatus } from "@/types";

type InputFormProps = {
  query: string;
  setQuery: (query: string) => void;
  handleSubmit: (e: React.FormEvent) => void;
  isProcessing: boolean;
  serverStatus: ServerStatus;
};

const InputForm: React.FC<InputFormProps> = ({
  query,
  setQuery,
  handleSubmit,
  isProcessing,
  serverStatus,
}) => {
  return (
    <form onSubmit={handleSubmit} className="flex gap-2 flex-shrink-0">
      <Input
        type="text"
        placeholder={
          isProcessing ? "Processing..." : "Enter your query..."
        }
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        disabled={isProcessing || serverStatus !== "connected"}
        className="flex-grow"
      />
      <Button
        type="submit"
        disabled={
          isProcessing || serverStatus !== "connected" || !query.trim()
        }
      >
        <Send size={18} />
      </Button>
    </form>
  );
};

export default InputForm;