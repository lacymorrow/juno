import { PermissionsFlow } from "@/components/PermissionsFlow";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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
  Keyboard,
  Mic,
  Shield,
  Sparkles,
  Star,
  Volume2,
  Zap,
} from "lucide-react";
import React, { useEffect, useState } from "react";

interface OnboardingFlowProps {
  onComplete: () => void;
  onSkip?: () => void;
  permissionsAlreadyGranted?: boolean;
}

type OnboardingStep = "welcome" | "features" | "permissions" | "voice-setup";

interface FeatureCard {
  icon: React.ComponentType<{ className?: string }>;
  title: string;
  description: string;
  capabilities: string[];
  color: string;
}

export function OnboardingFlow({
  onComplete,
  onSkip,
  permissionsAlreadyGranted = false,
}: OnboardingFlowProps) {
  const [currentStep, setCurrentStep] = useState<OnboardingStep>("welcome");
  const [completedSteps, setCompletedSteps] = useState<Set<OnboardingStep>>(
    new Set()
  );
  const [demoBarExpanded, setDemoBarExpanded] = useState(false);

  // Dynamically determine steps based on permissions state
  const steps: OnboardingStep[] = permissionsAlreadyGranted
    ? ["welcome", "features", "voice-setup"] // Skip permissions step
    : ["welcome", "features", "permissions", "voice-setup"]; // Include permissions step

  const currentStepIndex = steps.indexOf(currentStep);
  const progress = ((currentStepIndex + 1) / steps.length) * 100;

  // Mark current step as completed when moving forward
  const markStepCompleted = (step: OnboardingStep) => {
    setCompletedSteps((prev) => new Set([...prev, step]));
  };

  const nextStep = () => {
    markStepCompleted(currentStep);
    const nextIndex = currentStepIndex + 1;
    if (nextIndex < steps.length) {
      setCurrentStep(steps[nextIndex]);
    } else {
      // Reached the end, complete onboarding
      onComplete();
    }
  };

  const prevStep = () => {
    const prevIndex = currentStepIndex - 1;
    if (prevIndex >= 0) {
      setCurrentStep(steps[prevIndex]);
    }
  };

  const skipToStep = (step: OnboardingStep) => {
    // Prevent navigation to permissions step if already granted
    if (step === "permissions" && permissionsAlreadyGranted) {
      return;
    }
    setCurrentStep(step);
  };

  // Auto-mark permissions as completed if already granted
  useEffect(() => {
    if (permissionsAlreadyGranted) {
      setCompletedSteps((prev) => new Set([...prev, "permissions"]));
    }
  }, [permissionsAlreadyGranted]);

  // Reset demo bar state when leaving welcome step
  useEffect(() => {
    if (currentStep !== "welcome") {
      setDemoBarExpanded(false);
    }
  }, [currentStep]);

  // Feature cards data
  const featureCards: FeatureCard[] = [
    {
      icon: Brain,
      title: "AI Computer Control",
      description:
        "Your AI agent can see your screen and control your computer like a human would",
      capabilities: [
        "Click buttons and navigate interfaces",
        "Type text and fill forms",
        "Take and analyze screenshots",
        "Control applications and windows",
      ],
      color: "blue",
    },
    {
      icon: Globe,
      title: "Web Automation",
      description:
        "Browse the web, search for information, and interact with websites automatically",
      capabilities: [
        "Open websites and search engines",
        "Extract information from pages",
        "Fill out web forms",
        "Navigate complex web interfaces",
      ],
      color: "green",
    },
    {
      icon: FileText,
      title: "File & Document Management",
      description:
        "Create, edit, and organize files and documents across your system",
      capabilities: [
        "Create and edit text files",
        "Organize folders and files",
        "Open and work with documents",
        "Search and manage content",
      ],
      color: "purple",
    },
    {
      icon: Mic,
      title: "Voice Control",
      description: "Talk to Juno using natural speech - no typing required",
      capabilities: [
        "Voice commands for any task",
        "Dictation mode for text input",
        "Natural conversation interface",
        "Audio responses and feedback",
      ],
      color: "orange",
    },
  ];

  // Helper function to get explicit color classes to prevent layout shifts
  const getFeatureCardClasses = (color: string) => {
    const colorClasses = {
      blue: "border-blue-200 hover:border-blue-300",
      green: "border-green-200 hover:border-green-300",
      purple: "border-purple-200 hover:border-purple-300",
      orange: "border-orange-200 hover:border-orange-300",
    };
    return (
      colorClasses[color as keyof typeof colorClasses] ||
      "border-gray-200 hover:border-gray-300"
    );
  };

  const getIconClasses = (color: string) => {
    const iconClasses = {
      blue: "bg-blue-100 text-blue-600",
      green: "bg-green-100 text-green-600",
      purple: "bg-purple-100 text-purple-600",
      orange: "bg-orange-100 text-orange-600",
    };
    return (
      iconClasses[color as keyof typeof iconClasses] ||
      "bg-gray-100 text-gray-600"
    );
  };

  // Render individual steps
  const renderWelcomeStep = () => (
    <div className="max-w-2xl mx-auto text-center space-y-8">
      <div className="space-y-4">
        <div className="flex items-center justify-center mb-6">
          <div className="relative">
            <div className="w-20 h-20 rounded-full bg-gradient-to-br from-blue-500 to-purple-600 flex items-center justify-center">
              <Sparkles className="w-10 h-10 text-white" />
            </div>
            <div className="absolute -top-1 -right-1 w-6 h-6 bg-yellow-400 rounded-full flex items-center justify-center">
              <Star className="w-3 h-3 text-yellow-800" />
            </div>
          </div>
        </div>

        <h1 className="text-3xl font-bold text-foreground">
          Welcome to Juno AI
        </h1>
        <p className="text-lg text-muted-foreground max-w-xl mx-auto">
          Your intelligent desktop companion that can see, understand, and
          control your computer like a human would.
        </p>
      </div>

      {/* FloatingBar Demo Section */}
      <Card className="border-2 border-purple-200 bg-purple-50/50">
        <CardContent className="p-6">
          <div className="text-center space-y-4">
            <h3 className="font-semibold text-purple-900 mb-3">
              Meet Your AI Assistant
            </h3>
            <p className="text-sm text-purple-800 mb-4">
              This is Juno's main interface - a simple bar that's always ready
              to help
            </p>

            {/* Demo FloatingBar - Static Preview */}
            <div className="flex justify-center mb-4">
              <div className="relative bg-background rounded-lg border-2 border-dashed border-purple-300 p-8 w-80 h-20 flex items-center justify-center">
                <div
                  className={`flex items-center justify-center bg-black/90 text-white rounded-full shadow-lg border border-white/20 backdrop-blur-md transition-all duration-300 hover:scale-105 cursor-pointer ${
                    demoBarExpanded
                      ? "h-[40px] w-[240px] px-3"
                      : "h-[20px] w-[60px] px-2"
                  }`}
                  onClick={() => setDemoBarExpanded(!demoBarExpanded)}
                >
                  {demoBarExpanded ? (
                    <div className="flex items-center justify-between w-full h-full">
                      <input
                        type="text"
                        placeholder="Type a command..."
                        className="flex-1 bg-transparent border-none outline-none text-sm text-white placeholder-white/50"
                        readOnly
                      />
                      <button className="text-muted-foreground hover:text-white flex items-center justify-center h-6 w-6 transition-colors duration-200">
                        <svg
                          width="12"
                          height="12"
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                          strokeWidth="2"
                          strokeLinecap="round"
                          strokeLinejoin="round"
                        >
                          <path d="m22 2-7 20-4-9-9-4Z" />
                          <path d="M22 2 11 13" />
                        </svg>
                      </button>
                    </div>
                  ) : (
                    <div className="w-5 h-[4px] bg-emerald-400 rounded-full"></div>
                  )}
                </div>
                <div className="absolute -top-2 -left-2 text-xs text-purple-600 bg-purple-100 px-2 py-1 rounded">
                  {demoBarExpanded ? "Click to collapse!" : "Click to expand!"}
                </div>
              </div>
            </div>

            <div className="text-left space-y-2 max-w-sm mx-auto">
              <div className="flex items-center gap-2 text-sm text-purple-800">
                <div className="w-2 h-2 bg-purple-500 rounded-full flex-shrink-0"></div>
                <span>Click the bar to give voice or text commands</span>
              </div>
              <div className="flex items-center gap-2 text-sm text-purple-800">
                <div className="w-2 h-2 bg-purple-500 rounded-full flex-shrink-0"></div>
                <span>Stays out of your way until you need it</span>
              </div>
              <div className="flex items-center gap-2 text-sm text-purple-800">
                <div className="w-2 h-2 bg-purple-500 rounded-full flex-shrink-0"></div>
                <span>Works with any app or website</span>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card className="border-2 border-blue-200 bg-blue-50/50">
        <CardContent className="p-6">
          <div className="flex items-start gap-4">
            <div className="p-3 rounded-full bg-blue-100">
              <Zap className="w-6 h-6 text-blue-600" />
            </div>
            <div className="text-left">
              <h3 className="font-semibold text-blue-900 mb-2">
                What makes Juno special?
              </h3>
              <ul className="text-sm text-blue-800 space-y-1">
                <li>
                  • Uses Computer Vision to see and understand your screen
                </li>
                <li>• Performs tasks exactly like a human would</li>
                <li>• Works with any application or website</li>
                <li>• Responds to natural voice commands</li>
              </ul>
            </div>
          </div>
        </CardContent>
      </Card>

      <div className="flex gap-4 justify-center">
        <Button onClick={nextStep} size="lg" className="px-8">
          Get Started <ArrowRight className="w-4 h-4 ml-2" />
        </Button>
        {onSkip && (
          <Button variant="outline" onClick={onSkip} size="lg">
            Skip Setup
          </Button>
        )}
      </div>
    </div>
  );

  const renderFeaturesStep = () => (
    <div className="max-w-4xl mx-auto space-y-8">
      <div className="text-center space-y-4">
        <h2 className="text-2xl font-bold text-foreground">What Juno Can Do</h2>
        <p className="text-muted-foreground">
          Explore the powerful capabilities that make Juno your ultimate AI
          assistant
        </p>
      </div>

      {/* Permissions already granted notice */}
      {permissionsAlreadyGranted && (
        <Alert className="border-green-200 bg-green-50/50">
          <CheckCircle className="h-4 w-4 text-green-600" />
          <AlertDescription>
            <strong>Great news!</strong> Permissions are already configured.
            We'll skip the permissions setup and go straight to voice features.
          </AlertDescription>
        </Alert>
      )}

      <div className="grid md:grid-cols-2 gap-6">
        {featureCards.map((feature, index) => (
          <Card
            key={index}
            className={`border-2 transition-colors duration-200 hover:shadow-lg ${getFeatureCardClasses(
              feature.color
            )}`}
          >
            <CardHeader>
              <div className="flex items-center gap-3">
                <div
                  className={`p-3 rounded-lg ${getIconClasses(feature.color)}`}
                >
                  <feature.icon className="w-6 h-6" />
                </div>
                <CardTitle className="text-lg">{feature.title}</CardTitle>
              </div>
              <CardDescription>{feature.description}</CardDescription>
            </CardHeader>
            <CardContent>
              <div className="space-y-2">
                {feature.capabilities.map((capability, capIndex) => (
                  <div key={capIndex} className="flex items-center gap-2">
                    <CheckCircle className="w-4 h-4 text-green-500 flex-shrink-0" />
                    <span className="text-sm text-muted-foreground">
                      {capability}
                    </span>
                  </div>
                ))}
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      <div className="flex gap-4 justify-center">
        <Button variant="outline" onClick={prevStep}>
          <ArrowLeft className="w-4 h-4 mr-2" /> Back
        </Button>
        <Button onClick={nextStep} size="lg">
          Continue Setup <ArrowRight className="w-4 h-4 ml-2" />
        </Button>
      </div>
    </div>
  );

  const renderPermissionsStep = () => (
    <div className="max-w-3xl mx-auto space-y-6">
      <div className="text-center space-y-4">
        <div className="flex items-center justify-center">
          <div className="p-4 rounded-full bg-red-100">
            <Shield className="w-8 h-8 text-red-600" />
          </div>
        </div>
        <h2 className="text-2xl font-bold text-foreground">
          Security Permissions
        </h2>
        <p className="text-muted-foreground">
          Juno needs these permissions to control your computer safely and
          securely
        </p>
      </div>

      <Alert>
        <Info className="h-4 w-4" />
        <AlertDescription>
          <strong>Your privacy matters:</strong> All processing happens locally
          on your device. Juno only requests the minimum permissions needed to
          function.
        </AlertDescription>
      </Alert>

      <PermissionsFlow
        onComplete={() => {
          markStepCompleted("permissions");
          setTimeout(() => {
            setCurrentStep("voice-setup");
          }, 1000);
        }}
        showSkipOption={true}
        onSkip={() => {
          markStepCompleted("permissions");
          setCurrentStep("voice-setup");
        }}
        autoRedirectEnabled={false}
      />

      <div className="flex gap-4 justify-center">
        <Button variant="outline" onClick={prevStep}>
          <ArrowLeft className="w-4 h-4 mr-2" /> Back
        </Button>
      </div>
    </div>
  );

  const renderVoiceSetupStep = () => (
    <div className="max-w-2xl mx-auto space-y-8">
      <div className="text-center space-y-4">
        <div className="flex items-center justify-center">
          <div className="p-4 rounded-full bg-orange-100">
            <Volume2 className="w-8 h-8 text-orange-600" />
          </div>
        </div>
        <h2 className="text-2xl font-bold text-foreground">Voice Features</h2>
        <p className="text-muted-foreground">
          Control Juno with your voice for a hands-free experience
        </p>
      </div>

      <div className="grid gap-6">
        <Card className="border-2 border-orange-200 bg-orange-50/50">
          <CardHeader>
            <div className="flex items-center gap-3">
              <Mic className="w-6 h-6 text-orange-600" />
              <CardTitle>Voice Commands</CardTitle>
            </div>
            <CardDescription>
              Speak naturally to Juno and watch it perform tasks on your
              computer
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              <div className="p-3 rounded-lg bg-white border">
                <p className="text-sm font-medium">Try saying:</p>
                <p className="text-sm text-muted-foreground italic">
                  "Take a screenshot and open my email"
                </p>
              </div>
              <div className="p-3 rounded-lg bg-white border">
                <p className="text-sm font-medium">Or:</p>
                <p className="text-sm text-muted-foreground italic">
                  "Search Google for the weather forecast"
                </p>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card className="border-2 border-blue-200 bg-blue-50/50">
          <CardHeader>
            <div className="flex items-center gap-3">
              <Keyboard className="w-6 h-6 text-blue-600" />
              <CardTitle>Dictation Mode</CardTitle>
            </div>
            <CardDescription>
              Hold a key to dictate text directly into any application
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              <Badge variant="outline" className="bg-blue-100 text-blue-800">
                Default: Hold Fn key to dictate
              </Badge>
              <p className="text-sm text-muted-foreground">
                You can customize this shortcut in Settings later
              </p>
            </div>
          </CardContent>
        </Card>

        {/* Live voice status indicator */}
        <div className="p-4 rounded-lg border bg-muted/30">
          <h4 className="font-medium mb-3">Current Voice Status:</h4>
          <VoiceStatusIndicator variant="detailed" />
        </div>
      </div>

      <div className="flex gap-4 justify-center">
        <Button variant="outline" onClick={prevStep}>
          <ArrowLeft className="w-4 h-4 mr-2" /> Back
        </Button>
        <Button
          onClick={nextStep}
          size="lg"
          className="bg-gradient-to-r from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700"
        >
          Complete Setup <CheckCircle className="w-4 h-4 ml-2" />
        </Button>
      </div>
    </div>
  );

  // Main render
  return (
    <div className="min-h-screen bg-background p-6">
      <div className="max-w-6xl mx-auto">
        {/* Progress header */}
        <div className="mb-8">
          <div className="flex items-center justify-between mb-4">
            <h1 className="text-xl font-semibold text-foreground">
              Setup Juno AI
            </h1>
            <Badge variant="outline">
              Step {currentStepIndex + 1} of {steps.length}
            </Badge>
          </div>
          <Progress value={progress} className="h-2" />
        </div>

        {/* Step content */}
        <div className="mb-8">
          {currentStep === "welcome" && renderWelcomeStep()}
          {currentStep === "features" && renderFeaturesStep()}
          {currentStep === "permissions" && renderPermissionsStep()}
          {currentStep === "voice-setup" && renderVoiceSetupStep()}
        </div>

        {/* Step indicators */}
        <div className="flex justify-center gap-2">
          {steps.map((step, index) => (
            <button
              key={step}
              onClick={() =>
                index <= currentStepIndex ? skipToStep(step) : undefined
              }
              className={`w-3 h-3 rounded-full transition-colors duration-200 ${
                index === currentStepIndex
                  ? "bg-primary ring-2 ring-primary/20"
                  : completedSteps.has(step) ||
                    (step === "permissions" && permissionsAlreadyGranted)
                  ? "bg-green-500"
                  : index < currentStepIndex
                  ? "bg-muted-foreground/30 hover:bg-muted-foreground/50 cursor-pointer"
                  : "bg-muted-foreground/20"
              }`}
              disabled={index > currentStepIndex}
              title={
                step === "permissions" && permissionsAlreadyGranted
                  ? "Permissions (already granted - skipped)"
                  : step.charAt(0).toUpperCase() +
                    step.slice(1).replace("-", " ")
              }
            />
          ))}
        </div>
      </div>
    </div>
  );
}
