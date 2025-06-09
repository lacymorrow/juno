import React from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
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
      title: "Browse the Web",
      prompt: "Open Google and search for the latest macOS updates",
      category: "web"
    },
    {
      icon: Monitor,
      title: "Desktop Automation",
      prompt: "Take a screenshot and open System Preferences",
      category: "desktop"
    },
    {
      icon: FileText,
      title: "Create a Document",
      prompt: "Create a new note about my daily goals and open it",
      category: "files"
    },
    {
      icon: Code,
      title: "Development Help",
      prompt: "Help me understand the current project structure",
      category: "coding"
    },
    {
      icon: Music,
      title: "Entertainment",
      prompt: "Open Spotify and play some focus music",
      category: "apps"
    },
    {
      icon: Camera,
      title: "Visual Tasks",
      prompt: "Take a screenshot and describe what's on my screen",
      category: "visual"
    },
    {
      icon: Search,
      title: "Research",
      prompt: "Research the benefits of AI-human collaboration",
      category: "research"
    },
    {
      icon: MessageSquare,
      title: "General Chat",
      prompt: "What can you help me with today?",
      category: "general"
    }
  ];

  return (
    <div className="space-y-4">
      <div className="text-center space-y-2">
        <h3 className="text-lg font-semibold text-foreground">
          Try asking me to...
        </h3>
        <p className="text-sm text-muted-foreground">
          Click any example below to get started
        </p>
      </div>
      
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3 max-w-4xl mx-auto">
        {examplePrompts.map((example, index) => (
          <Card 
            key={index}
            className="cursor-pointer transition-all duration-200 hover:shadow-md hover:scale-[1.02] border border-border/50 hover:border-border"
          >
            <CardContent className="p-4">
              <Button
                variant="ghost"
                className="w-full h-auto p-0 justify-start text-left"
                onClick={() => onPromptSelect(example.prompt)}
              >
                <div className="flex items-start gap-3 w-full">
                  <div className="mt-1 p-2 rounded-lg bg-primary/10 text-primary flex-shrink-0">
                    <example.icon size={16} />
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="font-medium text-sm text-foreground mb-1">
                      {example.title}
                    </div>
                    <div className="text-xs text-muted-foreground line-clamp-2">
                      "{example.prompt}"
                    </div>
                  </div>
                </div>
              </Button>
            </CardContent>
          </Card>
        ))}
      </div>

      <div className="text-center pt-2">
        <p className="text-xs text-muted-foreground">
          Or type your own message below to start chatting
        </p>
      </div>
    </div>
  );
};