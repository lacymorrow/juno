import React from "react";
import { Button } from "@/components/ui/button";
import { 
  Globe, 
  Monitor, 
  FileText, 
  Code, 
  Music, 
  Camera, 
  Search,
  MessageSquare
} from "lucide-react";

interface ExamplePromptsProps {
  onPromptSelect: (prompt: string) => void;
}

export const ExamplePrompts: React.FC<ExamplePromptsProps> = ({ onPromptSelect }) => {
  const examplePrompts = [
    {
      icon: Globe,
      title: "Browse Web",
      prompt: "Open Google and search for the latest macOS updates",
    },
    {
      icon: Monitor,
      title: "Screenshot",
      prompt: "Take a screenshot and open System Preferences",
    },
    {
      icon: FileText,
      title: "Create Note",
      prompt: "Create a new note about my daily goals and open it",
    },
    {
      icon: Code,
      title: "Code Help",
      prompt: "Help me understand the current project structure",
    },
    {
      icon: Music,
      title: "Play Music",
      prompt: "Open Spotify and play some focus music",
    },
    {
      icon: Camera,
      title: "Describe Screen",
      prompt: "Take a screenshot and describe what's on my screen",
    },
    {
      icon: Search,
      title: "Research",
      prompt: "Research the benefits of AI-human collaboration",
    },
    {
      icon: MessageSquare,
      title: "General Chat",
      prompt: "What can you help me with today?",
    }
  ];

  return (
    <div className="space-y-1">
      <div className="grid grid-cols-2 gap-2 max-w-lg mx-auto">
        {examplePrompts.map((example, index) => (
          <Button
            key={index}
            variant="outline"
            className="h-auto p-2 justify-start text-left hover:bg-accent"
            onClick={() => onPromptSelect(example.prompt)}
          >
            <div className="flex items-center gap-2 w-full">
              <div className="p-1 rounded bg-primary/10 text-primary flex-shrink-0">
                <example.icon size={10} />
              </div>
              <div className="flex-1 min-w-0">
                <div className="text-xs font-medium text-foreground truncate">
                  {example.title}
                </div>
              </div>
            </div>
          </Button>
        ))}
      </div>
    </div>
  );
};