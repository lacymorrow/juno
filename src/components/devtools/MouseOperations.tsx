import React, { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { MousePointer, Hand } from "lucide-react";
import { invokeCommand } from "@/lib/utils";

const MouseOperations: React.FC = () => {
  const [mouseX, setMouseX] = useState<string>("");
  const [mouseY, setMouseY] = useState<string>("");
  const [dragStartX, setDragStartX] = useState<string>("");
  const [dragStartY, setDragStartY] = useState<string>("");
  const [dragEndX, setDragEndX] = useState<string>("");
  const [dragEndY, setDragEndY] = useState<string>("");

  // Show consolidation info
  const showConsolidationInfo = () => {
    toast.info(
      "🔧 Mouse Tools Consolidated! Now using Computer Tool:\n" +
        '• Move: computer tool with action: "move"\n' +
        '• Click: computer tool with action: "click"\n' +
        '• Right-click: computer tool with action: "right_click"\n' +
        '• Middle-click: computer tool with action: "middle_click"\n' +
        '• Drag: computer tool with action: "left_click_drag"',
      { duration: 10000 }
    );
  };

  const handleMouseMove = async () => {
    const x = parseInt(mouseX, 10);
    const y = parseInt(mouseY, 10);

    if (isNaN(x)) {
      toast.error("Invalid X coordinate. Please enter a number.");
      return;
    }
    if (isNaN(y)) {
      toast.error("Invalid Y coordinate. Please enter a number.");
      return;
    }

    await invokeCommand(
      "computer",
      { action: "move", coordinate: [x, y] },
      "mouseMove"
    );
  };

  const handleRightClick = async () => {
    const x = parseInt(mouseX, 10);
    const y = parseInt(mouseY, 10);

    if (isNaN(x)) {
      toast.error("Invalid X coordinate. Please enter a number.");
      return;
    }
    if (isNaN(y)) {
      toast.error("Invalid Y coordinate. Please enter a number.");
      return;
    }

    await invokeCommand(
      "computer",
      { action: "right_click", coordinate: [x, y] },
      "rightClick"
    );
  };

  const handleMiddleClick = async () => {
    const x = parseInt(mouseX, 10);
    const y = parseInt(mouseY, 10);

    if (isNaN(x)) {
      toast.error("Invalid X coordinate. Please enter a number.");
      return;
    }
    if (isNaN(y)) {
      toast.error("Invalid Y coordinate. Please enter a number.");
      return;
    }

    await invokeCommand(
      "computer",
      { action: "middle_click", coordinate: [x, y] },
      "middleClick"
    );
  };

  const handleMouseClick = async () => {
    const x = parseInt(mouseX, 10);
    const y = parseInt(mouseY, 10);

    if (isNaN(x)) {
      toast.error("Invalid X coordinate. Please enter a number.");
      return;
    }
    if (isNaN(y)) {
      toast.error("Invalid Y coordinate. Please enter a number.");
      return;
    }

    await invokeCommand(
      "computer",
      { action: "click", coordinate: [x, y] },
      "mouseClick"
    );
  };

  // Double-click action not exposed

  const handleMouseDrag = async () => {
    const startX = parseInt(dragStartX, 10);
    const startY = parseInt(dragStartY, 10);
    const endX = parseInt(dragEndX, 10);
    const endY = parseInt(dragEndY, 10);

    if (isNaN(startX) || isNaN(startY) || isNaN(endX) || isNaN(endY)) {
      toast.error("Invalid coordinates. Please enter numbers for all fields.");
      return;
    }

    await invokeCommand(
      "computer",
      {
        action: "drag",
        startCoordinate: [startX, startY],
        endCoordinate: [endX, endY],
      },
      "mouseDrag"
    );
  };

  return (
    <div className="space-y-4">
      <div className="bg-blue-50 dark:bg-blue-900/20 p-3 rounded-lg">
        <p className="text-sm text-blue-700 dark:text-blue-300 flex items-center gap-2">
          <Hand className="h-4 w-4" />
          Now using consolidated Computer Tool! Click info for details.
        </p>
        <Button
          variant="outline"
          size="sm"
          onClick={showConsolidationInfo}
          className="mt-2"
        >
          Show Consolidation Info
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
          <Button onClick={handleMouseClick}>Click</Button>
          <Button onClick={handleRightClick}>Right Click</Button>
          <Button onClick={handleMiddleClick}>Middle Click</Button>
          {/* Double Click removed from devtools */}
        </div>
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <Hand className="h-4 w-4" />
          <span className="text-sm">Drag coordinates:</span>
        </div>
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
  );
};

export default MouseOperations;
