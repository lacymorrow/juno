import React, { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Keyboard, Clipboard, Type } from "lucide-react";
import { invokeCommand } from "@/lib/utils";

const KeyboardOperations: React.FC = () => {
  const [textToType, setTextToType] = useState<string>("");
  const [globalTextToType, setGlobalTextToType] = useState<string>("");
  const [keyToRelease, setKeyToRelease] = useState<string>("");
  const [clipboardContent, setClipboardContent] = useState<string>("");
  const [clipboardResult, setClipboardResult] = useState<string | null>(null);
  const [selectedTextResult, setSelectedTextResult] = useState<string | null>(
    null
  );

  const handleTypeText = async () => {
    if (!textToType.trim()) {
      toast.error("Please enter text to type.");
      return;
    }
    await invokeCommand("type_text", { text: textToType }, "typeText");
  };

  const handleGlobalTypeText = async () => {
    if (!globalTextToType.trim()) {
      toast.error("Please enter text to type globally.");
      return;
    }
    await invokeCommand(
      "global_type_text",
      { text: globalTextToType },
      "globalTypeText"
    );
  };

  const handleReleaseKey = async () => {
    if (!keyToRelease.trim()) {
      toast.error("Please enter a key to release.");
      return;
    }
    await invokeCommand("release_key", { key: keyToRelease }, "releaseKey");
  };

  const handleGetClipboard = async () => {
    setClipboardResult(null);
    const result = await invokeCommand<string | null>(
      "get_clipboard",
      {},
      "getClipboard"
    );
    if (result !== null) {
      setClipboardResult(result);
    }
  };

  const handleSetClipboard = async () => {
    if (!clipboardContent.trim()) {
      toast.error("Please enter content to set to clipboard.");
      return;
    }
    await invokeCommand(
      "set_clipboard",
      { content: clipboardContent },
      "setClipboard"
    );
  };

  const handleGetSelectedText = async () => {
    setSelectedTextResult(null);
    const result = await invokeCommand<string | null>(
      "get_selected_text",
      {},
      "getSelectedText"
    );
    if (result !== null) {
      setSelectedTextResult(result);
    }
  };

  return (
    <div className="space-y-4">
      <div className="bg-green-50 dark:bg-green-900/20 p-3 rounded-lg">
        <p className="text-sm text-green-700 dark:text-green-300 flex items-center gap-2">
          <Type className="h-4 w-4" />
          Using production keyboard functions with debug capabilities!
        </p>
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <Keyboard className="h-4 w-4" />
          <Input
            placeholder="Text to type"
            value={textToType}
            onChange={(e) => setTextToType(e.target.value)}
          />
          <Button onClick={handleTypeText}>Type Text</Button>
        </div>
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <Type className="h-4 w-4" />
          <Input
            placeholder="Text to type globally"
            value={globalTextToType}
            onChange={(e) => setGlobalTextToType(e.target.value)}
          />
          <Button onClick={handleGlobalTypeText}>Global Type</Button>
        </div>
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <Keyboard className="h-4 w-4" />
          <Input
            placeholder="Key to release (e.g., shift, cmd)"
            value={keyToRelease}
            onChange={(e) => setKeyToRelease(e.target.value)}
          />
          <Button onClick={handleReleaseKey}>Release Key</Button>
        </div>
        <p className="text-xs text-gray-500">
          Note: Release key provides unique functionality not available in
          computer tool
        </p>
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <Clipboard className="h-4 w-4" />
          <Button onClick={handleGetClipboard}>Get Clipboard</Button>
        </div>
        {clipboardResult && (
          <pre className="mt-2 whitespace-pre-wrap break-all text-sm bg-gray-100 dark:bg-gray-800 p-2 rounded">
            {clipboardResult}
          </pre>
        )}
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <Clipboard className="h-4 w-4" />
          <Input
            placeholder="Content to set to clipboard"
            value={clipboardContent}
            onChange={(e) => setClipboardContent(e.target.value)}
          />
          <Button onClick={handleSetClipboard}>Set Clipboard</Button>
        </div>
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <Type className="h-4 w-4" />
          <Button onClick={handleGetSelectedText}>Get Selected Text</Button>
        </div>
        {selectedTextResult && (
          <pre className="mt-2 whitespace-pre-wrap break-all text-sm bg-gray-100 dark:bg-gray-800 p-2 rounded">
            {selectedTextResult}
          </pre>
        )}
      </div>
    </div>
  );
};

export default KeyboardOperations;
