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
      gradient: "from-blue-500 to-cyan-500",
      bgGradient: "from-blue-50/50 to-cyan-50/30 dark:from-blue-950/30 dark:to-cyan-950/20",
    },
    {
      icon: Monitor,
      title: "Screenshot",
      prompt: "Take a screenshot and open System Preferences",
      gradient: "from-purple-500 to-violet-500",
      bgGradient: "from-purple-50/50 to-violet-50/30 dark:from-purple-950/30 dark:to-violet-950/20",
    },
    {
      icon: FileText,
      title: "Create Note",
      prompt: "Create a new note about my daily goals and open it",
      gradient: "from-green-500 to-emerald-500",
      bgGradient: "from-green-50/50 to-emerald-50/30 dark:from-green-950/30 dark:to-emerald-950/20",
    },
    {
      icon: Code,
      title: "Code Help",
      prompt: "Help me understand the current project structure",
      gradient: "from-orange-500 to-amber-500",
      bgGradient: "from-orange-50/50 to-amber-50/30 dark:from-orange-950/30 dark:to-amber-950/20",
    },
    {
      icon: Music,
      title: "Play Music",
      prompt: "Open Spotify and play some focus music",
      gradient: "from-pink-500 to-rose-500",
      bgGradient: "from-pink-50/50 to-rose-50/30 dark:from-pink-950/30 dark:to-rose-950/20",
    },
    {
      icon: Camera,
      title: "Describe Screen",
      prompt: "Take a screenshot and describe what's on my screen",
      gradient: "from-indigo-500 to-blue-500",
      bgGradient: "from-indigo-50/50 to-blue-50/30 dark:from-indigo-950/30 dark:to-blue-950/20",
    },
    {
      icon: Search,
      title: "Research",
      prompt: "Research the benefits of AI-human collaboration",
      gradient: "from-teal-500 to-cyan-500",
      bgGradient: "from-teal-50/50 to-cyan-50/30 dark:from-teal-950/30 dark:to-cyan-950/20",
    },
    {
      icon: MessageSquare,
      title: "General Chat",
      prompt: "What can you help me with today?",
      gradient: "from-slate-500 to-gray-500",
      bgGradient: "from-slate-50/50 to-gray-50/30 dark:from-slate-950/30 dark:to-gray-950/20",
    }
  ];

  return (
    <div className="space-y-4">
      <div className="text-center space-y-2">
        <h3 className="text-lg font-semibold bg-gradient-to-r from-purple-700 to-indigo-700 dark:from-purple-300 dark:to-indigo-300 bg-clip-text text-transparent">
          Try these examples
        </h3>
        <p className="text-sm text-muted-foreground">
          Click any prompt to get started with Juno AI
        </p>
      </div>

      <div className="grid grid-cols-2 gap-3 max-w-2xl mx-auto">
        {examplePrompts.map((example, index) => (
          <Button
            key={index}
            variant="outline"
            className={`h-auto p-4 justify-start text-left hover:shadow-md transition-all duration-200 rounded-xl bg-gradient-to-r ${example.bgGradient} border-border/30 hover:border-border/50 backdrop-blur-sm group`}
            onClick={() => onPromptSelect(example.prompt)}
          >
            <div className="flex items-center gap-3 w-full">
              <div className={`p-2 rounded-lg bg-gradient-to-r ${example.gradient} text-white flex-shrink-0 group-hover:scale-105 transition-transform duration-200 shadow-sm`}>
                <example.icon size={16} />
              </div>
              <div className="flex-1 min-w-0">
                <div className="text-sm font-medium text-foreground group-hover:text-foreground/90 transition-colors duration-200">
                  {example.title}
                </div>
                <div className="text-xs text-muted-foreground mt-1 line-clamp-2 leading-relaxed">
                  {example.prompt}
                </div>
              </div>
            </div>
          </Button>
        ))}
      </div>
    </div>
  );
};
