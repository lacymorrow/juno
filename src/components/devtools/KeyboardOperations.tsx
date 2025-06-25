import React, { useState } from 'react';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Keyboard, ClipboardPaste, TextSelect } from 'lucide-react';
import { invokeCommand } from '@/lib/utils';

const KeyboardOperations: React.FC = () => {
  const [textToType, setTextToType] = useState<string>('Hello from DevTools!');
  const [globalTextToType, setGlobalTextToType] = useState<string>('Global text');
  // REMOVED: keyToPress state - consolidated into computer tool
  const [modifierKey, setModifierKey] = useState<string>('shift');
  const [clipboardContent, setClipboardContent] = useState<string>('');
  const [clipboardResult, setClipboardResult] = useState<string | null>(null);
  const [selectedTextResult, setSelectedTextResult] = useState<string | null>(null);

  const handleTypeText = async () => {
    if (!textToType.trim()) {
      toast.error('Please enter text to type.');
      return;
    }
    await invokeCommand(
      'dev_type_text',
      { text: textToType },
      'typeText'
    );
  };

  const handleGlobalTypeText = async () => {
    if (!globalTextToType.trim()) {
      toast.error('Please enter text to type globally.');
      return;
    }
    await invokeCommand(
      'dev_global_type_text',
      { text: globalTextToType },
      'globalTypeText'
    );
  };

  // REMOVED: handlePressKey and handleHoldKey functions
  // These have been consolidated into the official Anthropic Computer Use API
  // Use the 'computer' tool with actions: "key" and "hold_key" instead
  // This eliminates redundancy and ensures API compliance

  const handleReleaseKey = async () => {
    if (!modifierKey.trim()) {
      toast.error('Please enter a modifier key to release.');
      return;
    }
    await invokeCommand(
      'dev_release_key',
      { key: modifierKey.trim() },
      'releaseKey'
    );
  };

  const handleGetClipboard = async () => {
    setClipboardResult(null);
    const result = await invokeCommand<string | null>(
      'dev_get_clipboard',
      {},
      'getClipboard'
    );
    if (result !== null) {
      setClipboardResult(result);
    }
  };

  const handleSetClipboard = async () => {
    if (!clipboardContent.trim()) {
      toast.error('Please enter content to set to clipboard.');
      return;
    }
    await invokeCommand(
      'dev_set_clipboard',
      { content: clipboardContent },
      'setClipboard'
    );
  };

  const handleGetSelectedText = async () => {
    setSelectedTextResult(null);
    const result = await invokeCommand<string | null>(
      'dev_get_selected_text',
      {},
      'getSelectedText'
    );
    if (result !== null) {
      setSelectedTextResult(result);
    }
  };

  return (
    <div className="space-y-4">
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
          <Keyboard className="h-4 w-4" />
          <Input
            placeholder="Global text to type"
            value={globalTextToType}
            onChange={(e) => setGlobalTextToType(e.target.value)}
          />
          <Button onClick={handleGlobalTypeText}>Type Globally</Button>
        </div>
      </div>

      {/* REMOVED: Press Key and Hold Key sections - consolidated into computer tool */}
      <div className="space-y-2">
        <div className="flex items-center space-x-2 text-sm text-muted-foreground">
          <Keyboard className="h-4 w-4" />
          <span>Press Key & Hold Key: Use 'computer' tool with actions "key" and "hold_key" instead</span>
        </div>
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <Keyboard className="h-4 w-4" />
          <Input
            placeholder="Modifier key (e.g., shift, ctrl)"
            value={modifierKey}
            onChange={(e) => setModifierKey(e.target.value)}
          />
          <Button onClick={handleReleaseKey}>Release Key</Button>
        </div>
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <ClipboardPaste className="h-4 w-4" />
          <Input
            placeholder="Content for clipboard"
            value={clipboardContent}
            onChange={(e) => setClipboardContent(e.target.value)}
          />
          <Button onClick={handleSetClipboard}>Set Clipboard</Button>
          <Button onClick={handleGetClipboard}>Get Clipboard</Button>
        </div>
        {clipboardResult && (
          <pre className="mt-2 whitespace-pre-wrap break-all text-sm">
            {clipboardResult}
          </pre>
        )}
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <TextSelect className="h-4 w-4" />
          <Button onClick={handleGetSelectedText}>Get Selected Text</Button>
        </div>
        {selectedTextResult && (
          <pre className="mt-2 whitespace-pre-wrap break-all text-sm">
            {selectedTextResult}
          </pre>
        )}
      </div>
    </div>
  );
};

export default KeyboardOperations;
