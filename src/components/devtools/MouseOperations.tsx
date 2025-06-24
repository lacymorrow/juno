import React, { useState } from 'react';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { MousePointer, Hand } from 'lucide-react';
import { invokeCommand } from '@/lib/utils';

const MouseOperations: React.FC = () => {
  const [mouseX, setMouseX] = useState<string>('');
  const [mouseY, setMouseY] = useState<string>('');
  const [dragStartX, setDragStartX] = useState<string>('');
  const [dragStartY, setDragStartY] = useState<string>('');
  const [dragEndX, setDragEndX] = useState<string>('');
  const [dragEndY, setDragEndY] = useState<string>('');

  // Show consolidation info
  const showConsolidationInfo = () => {
    toast.info(
      '🔧 Mouse Tools Consolidated! Use the Computer Tool instead:\n' +
      '• Click: computer tool with action: "click"\n' +
      '• Right-click: computer tool with action: "right_click"\n' +
      '• Double-click: computer tool with action: "double_click"\n' +
      '• Drag: computer tool with action: "drag"',
      { duration: 8000 }
    );
  };

  const handleMouseMove = async () => {
    const x = parseInt(mouseX, 10);
    const y = parseInt(mouseY, 10);

    if (isNaN(x)) {
      toast.error('Invalid X coordinate. Please enter a number.');
      return;
    }
    if (isNaN(y)) {
      toast.error('Invalid Y coordinate. Please enter a number.');
      return;
    }

    await invokeCommand(
      'dev_mouse_move',
      { x, y },
      'mouseMove'
    );
  };

  const handleMouseDown = async () => {
    const x = parseInt(mouseX, 10);
    const y = parseInt(mouseY, 10);

    if (isNaN(x)) {
      toast.error('Invalid X coordinate. Please enter a number.');
      return;
    }
    if (isNaN(y)) {
      toast.error('Invalid Y coordinate. Please enter a number.');
      return;
    }

    await invokeCommand(
      'dev_mouse_down',
      { x, y },
      'mouseDown'
    );
  };

  const handleMouseUp = async () => {
    const x = parseInt(mouseX, 10);
    const y = parseInt(mouseY, 10);

    if (isNaN(x)) {
      toast.error('Invalid X coordinate. Please enter a number.');
      return;
    }
    if (isNaN(y)) {
      toast.error('Invalid Y coordinate. Please enter a number.');
      return;
    }

    await invokeCommand(
      'dev_mouse_up',
      { x, y },
      'mouseUp'
    );
  };

  const handleMouseClick = async () => {
    const x = parseInt(mouseX, 10);
    const y = parseInt(mouseY, 10);

    if (isNaN(x)) {
      toast.error('Invalid X coordinate. Please enter a number.');
      return;
    }
    if (isNaN(y)) {
      toast.error('Invalid Y coordinate. Please enter a number.');
      return;
    }

    await invokeCommand(
      'dev_mouse_click',
      { x, y },
      'mouseClick'
    );
  };

  const handleMouseDoubleClick = async () => {
    const x = parseInt(mouseX, 10);
    const y = parseInt(mouseY, 10);

    if (isNaN(x)) {
      toast.error('Invalid X coordinate. Please enter a number.');
      return;
    }
    if (isNaN(y)) {
      toast.error('Invalid Y coordinate. Please enter a number.');
      return;
    }

    await invokeCommand(
      'dev_mouse_double_click',
      { x, y },
      'mouseDoubleClick'
    );
  };

  const handleMouseDrag = async () => {
    const startX = parseInt(dragStartX, 10);
    const startY = parseInt(dragStartY, 10);
    const endX = parseInt(dragEndX, 10);
    const endY = parseInt(dragEndY, 10);

    if (isNaN(startX) || isNaN(startY) || isNaN(endX) || isNaN(endY)) {
      toast.error('Invalid coordinates. Please enter numbers for all fields.');
      return;
    }

    await invokeCommand(
      'dev_mouse_drag',
      { startX, startY, endX, endY },
      'mouseDrag'
    );
  };

  return (
    <div className="space-y-4">
      {/* Consolidation Notice */}
      <div className="bg-blue-50 dark:bg-blue-950 border border-blue-200 dark:border-blue-800 rounded-lg p-4">
        <div className="flex items-center space-x-2 mb-2">
          <MousePointer className="h-5 w-5 text-blue-600 dark:text-blue-400" />
          <h3 className="font-semibold text-blue-800 dark:text-blue-200">Mouse Tools Consolidated</h3>
        </div>
        <p className="text-sm text-blue-700 dark:text-blue-300 mb-3">
          Mouse operations have been consolidated into the official <code className="bg-blue-100 dark:bg-blue-900 px-1 rounded">computer</code> tool for 100% Anthropic API compliance.
        </p>
        <div className="space-y-1 text-xs text-blue-600 dark:text-blue-400">
          <div>• <strong>Click:</strong> <code>computer</code> with <code>action: "click"</code></div>
          <div>• <strong>Right-click:</strong> <code>computer</code> with <code>action: "right_click"</code></div>
          <div>• <strong>Double-click:</strong> <code>computer</code> with <code>action: "double_click"</code></div>
          <div>• <strong>Drag:</strong> <code>computer</code> with <code>action: "drag"</code></div>
        </div>
        <Button
          onClick={showConsolidationInfo}
          variant="outline"
          size="sm"
          className="mt-3 text-blue-700 border-blue-300 hover:bg-blue-100 dark:text-blue-300 dark:border-blue-700 dark:hover:bg-blue-900"
        >
          Show Examples
        </Button>
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <MousePointer className="h-4 w-4" />
          <Input
            placeholder="X coordinate"
            value={mouseX}
            onChange={(e) => setMouseX(e.target.value)}
          />
          <Input
            placeholder="Y coordinate"
            value={mouseY}
            onChange={(e) => setMouseY(e.target.value)}
          />
        </div>
        <div className="flex space-x-2">
          <Button onClick={handleMouseMove}>Move</Button>
          <Button onClick={handleMouseDown}>Down</Button>
          <Button onClick={handleMouseUp}>Up</Button>
          <Button onClick={handleMouseClick}>Click</Button>
          <Button onClick={handleMouseDoubleClick}>Double Click</Button>
        </div>
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <Hand className="h-4 w-4" />
          <div className="grid grid-cols-2 gap-2">
            <Input
              placeholder="Start X"
              value={dragStartX}
              onChange={(e) => setDragStartX(e.target.value)}
            />
            <Input
              placeholder="Start Y"
              value={dragStartY}
              onChange={(e) => setDragStartY(e.target.value)}
            />
            <Input
              placeholder="End X"
              value={dragEndX}
              onChange={(e) => setDragEndX(e.target.value)}
            />
            <Input
              placeholder="End Y"
              value={dragEndY}
              onChange={(e) => setDragEndY(e.target.value)}
            />
          </div>
          <Button onClick={handleMouseDrag}>Drag</Button>
        </div>
      </div>
    </div>
  );
};

export default MouseOperations;
