import { PermissionsFlow } from "@/components/PermissionsFlow";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { VoiceStatusIndicator } from "@/components/VoiceStatusIndicator";
import {
  ArrowLeft,
  ArrowRight,
  Brain,
  CheckCircle,
  FileText,
  Globe,
  Info,
  Mic,
  Shield,
  Sparkles,
} from "lucide-react";
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

// Import the KeyboardShortcuts type
interface KeyboardShortcuts {
  agent_mode_toggle: string;
  dictation_input: string;
  stop_current_task: string;
  open_settings: string;
}

interface OnboardingFlowProps {
  onComplete: () => void;
  onSkip?: () => void;
  permissionsAlreadyGranted?: boolean;
}

type OnboardingStep = "welcome" | "permissions" | "complete";

export function OnboardingFlow({
  onComplete,
  onSkip,
  permissionsAlreadyGranted = false,
}: OnboardingFlowProps) {
  const [currentStep, setCurrentStep] = useState<OnboardingStep>("welcome");
  const [keyboardShortcuts, setKeyboardShortcuts] =
    useState<KeyboardShortcuts | null>(null);

  // Load current keyboard shortcuts on mount
  useEffect(() => {
    const loadShortcuts = async () => {
      try {
        const shortcuts = await invoke<KeyboardShortcuts>(
          "get_keyboard_shortcuts"
        );
        setKeyboardShortcuts(shortcuts);
      } catch (error) {
        console.error("Failed to load keyboard shortcuts:", error);
        // Keep defaults if loading fails
      }
    };

    loadShortcuts();
  }, []);

  // Dynamically determine steps based on permissions state
  const steps: OnboardingStep[] = permissionsAlreadyGranted
    ? ["welcome", "complete"] // Skip permissions step
    : ["welcome", "permissions", "complete"]; // Include permissions step

  const currentStepIndex = steps.indexOf(currentStep);
  const progress = ((currentStepIndex + 1) / steps.length) * 100;

  const nextStep = () => {
    const nextIndex = currentStepIndex + 1;
    if (nextIndex < steps.length) {
      setCurrentStep(steps[nextIndex]);
    } else {
      onComplete();
    }
  };

  const prevStep = () => {
    const prevIndex = currentStepIndex - 1;
    if (prevIndex >= 0) {
      setCurrentStep(steps[prevIndex]);
    }
  };

  // Quick feature overview
  const features = [
    {
      icon: Brain,
      title: "AI Computer Control",
      desc: "Control your computer with natural language",
    },
    {
      icon: Globe,
      title: "Web Automation",
      desc: "Browse and interact with websites automatically",
    },
    {
      icon: FileText,
      title: "File Management",
      desc: "Create, edit, and organize files and documents",
    },
    {
      icon: Mic,
      title: "Voice Control",
      desc: "Talk to Juno using natural speech commands",
    },
  ];

  const renderWelcomeStep = () => (
    <div className="max-w-3xl mx-auto text-center space-y-6">
      <div className="space-y-3">
        <div className="w-16 h-16 rounded-full bg-gradient-to-br from-blue-500 to-purple-600 flex items-center justify-center mx-auto">
          <Sparkles className="w-8 h-8 text-white" />
        </div>
        <h1 className="text-2xl font-bold text-foreground">
          Welcome to Juno AI
        </h1>
        <p className="text-muted-foreground">
          Your intelligent desktop assistant that can see and control your
          computer
        </p>
      </div>

      {/* Quick features grid */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4 my-8">
        {features.map((feature, index) => (
          <Card
            key={index}
            className="p-4 text-center border-2 hover:border-primary/50 transition-colors"
          >
            <feature.icon className="w-8 h-8 mx-auto mb-2 text-primary" />
            <h3 className="font-medium text-sm mb-1">{feature.title}</h3>
            <p className="text-xs text-muted-foreground">{feature.desc}</p>
          </Card>
        ))}
      </div>

      {/* Permissions already granted notice */}
      {permissionsAlreadyGranted && (
        <Alert className="border-green-200 bg-green-50/50 text-left">
          <CheckCircle className="h-4 w-4 text-green-600" />
          <AlertDescription>
            Permissions are already configured. You're ready to start using
            Juno!
          </AlertDescription>
        </Alert>
      )}

      {/* Voice shortcuts info - Now using dynamic shortcuts */}
      <Card className="border-2 border-blue-200 bg-blue-50/50 p-4">
        <div className="flex items-center justify-center gap-6 text-sm">
          <div className="flex items-center gap-2">
            <Badge variant="outline" className="bg-blue-100">
              {keyboardShortcuts?.agent_mode_toggle || "Loading..."}
            </Badge>
            <span className="text-blue-800">Agent Mode</span>
          </div>
          <div className="flex items-center gap-2">
            <Badge variant="outline" className="bg-blue-100">
              {keyboardShortcuts?.dictation_input || "Loading..."}
            </Badge>
            <span className="text-blue-800">Dictation Mode</span>
          </div>
        </div>
      </Card>

      <div className="flex gap-3 justify-center">
        <Button onClick={nextStep} size="lg" className="px-8">
          {permissionsAlreadyGranted ? "Get Started" : "Continue Setup"}
          <ArrowRight className="w-4 h-4 ml-2" />
        </Button>
        {onSkip && (
          <Button variant="outline" onClick={onSkip} size="lg">
            Skip Setup
          </Button>
        )}
      </div>
    </div>
  );

  const renderPermissionsStep = () => (
    <div className="max-w-2xl mx-auto space-y-6">
      <div className="text-center space-y-3">
        <div className="w-12 h-12 rounded-full bg-red-100 flex items-center justify-center mx-auto">
          <Shield className="w-6 h-6 text-red-600" />
        </div>
        <h2 className="text-xl font-bold text-foreground">Quick Setup</h2>
        <p className="text-muted-foreground">
          Juno needs accessibility permissions to control your computer
        </p>
      </div>

      <Alert>
        <Info className="h-4 w-4" />
        <AlertDescription>
          All processing happens locally on your device for maximum privacy and
          security.
        </AlertDescription>
      </Alert>

      <PermissionsFlow
        onComplete={() => {
          setTimeout(() => setCurrentStep("complete"), 800);
        }}
        showSkipOption={true}
        onSkip={() => setCurrentStep("complete")}
        autoRedirectEnabled={false}
      />

      <div className="flex gap-3 justify-center">
        <Button variant="outline" onClick={prevStep}>
          <ArrowLeft className="w-4 h-4 mr-2" /> Back
        </Button>
      </div>
    </div>
  );

  const renderCompleteStep = () => (
    <div className="max-w-2xl mx-auto text-center space-y-6">
      <div className="space-y-3">
        <div className="w-16 h-16 rounded-full bg-green-100 flex items-center justify-center mx-auto">
          <CheckCircle className="w-8 h-8 text-green-600" />
        </div>
        <h2 className="text-2xl font-bold text-foreground">All Set!</h2>
        <p className="text-muted-foreground">
          Juno is ready to help. Try these quick commands:
        </p>
      </div>

      <div className="space-y-3 text-left max-w-md mx-auto">
        <div className="p-3 rounded-lg bg-muted/30 border">
          <p className="text-sm font-medium">Say or type:</p>
          <p className="text-sm text-muted-foreground italic">
            "Take a screenshot"
          </p>
        </div>
        <div className="p-3 rounded-lg bg-muted/30 border">
          <p className="text-sm font-medium">Or try:</p>
          <p className="text-sm text-muted-foreground italic">
            "Open my email"
          </p>
        </div>
      </div>

      {/* Voice status */}
      <div className="p-4 rounded-lg border bg-muted/20">
        <h4 className="font-medium mb-2 text-sm">Voice Status:</h4>
        <VoiceStatusIndicator variant="detailed" />
      </div>

      <Button
        onClick={onComplete}
        size="lg"
        className="px-8 bg-gradient-to-r from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700"
      >
        Start Using Juno <CheckCircle className="w-4 h-4 ml-2" />
      </Button>
    </div>
  );

  return (
    <div className="min-h-screen bg-background p-6">
      <div className="max-w-4xl mx-auto">
        {/* Progress header */}
        <div className="mb-8">
          <div className="flex items-center justify-between mb-3">
            <h1 className="text-lg font-semibold text-foreground">
              Setup Juno AI
            </h1>
            <Badge variant="outline">
              {currentStepIndex + 1} of {steps.length}
            </Badge>
          </div>
          <Progress value={progress} className="h-2" />
        </div>

        {/* Step content */}
        <div className="mb-6">
          {currentStep === "welcome" && renderWelcomeStep()}
          {currentStep === "permissions" && renderPermissionsStep()}
          {currentStep === "complete" && renderCompleteStep()}
        </div>

        {/* Step indicators */}
        <div className="flex justify-center gap-2">
          {steps.map((step, index) => (
            <div
              key={step}
              className={`w-2 h-2 rounded-full transition-colors duration-200 ${
                index === currentStepIndex
                  ? "bg-primary"
                  : index < currentStepIndex
                  ? "bg-green-500"
                  : "bg-muted-foreground/30"
              }`}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
