import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { Save, CheckCircle } from "lucide-react";
import { SettingsSectionProps } from "../types";

export default function AIProviderSettings({ settings }: SettingsSectionProps) {
  const currentProvider = settings.providers.find(p => p.id === settings.activeProvider);

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">AI Provider</h3>

        <Card>
          <CardHeader>
            <CardTitle>Provider Selection</CardTitle>
            <CardDescription>Choose your AI provider and model</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="ai-provider">Active Provider</Label>
              <Select
                value={settings.activeProvider}
                onValueChange={settings.handleActiveProviderChange}
              >
                <SelectTrigger>
                  <SelectValue placeholder="Select AI provider" />
                </SelectTrigger>
                <SelectContent>
                  {settings.providers.map((provider) => (
                    <SelectItem key={provider.id} value={provider.id}>
                      <div className="flex items-center gap-2">
                        <span>{provider.name}</span>
                        {provider.computer_use_supported && (
                          <Badge variant="secondary" className="text-xs bg-blue-100 text-blue-800">
                            Computer Use
                          </Badge>
                        )}
                      </div>
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {currentProvider && (
                <div className="space-y-2">
                  <p className="text-sm text-muted-foreground">
                    {currentProvider.description}
                  </p>
                  {currentProvider.computer_use_supported && (
                    <div className="flex items-center gap-2 text-sm">
                      <CheckCircle className="h-4 w-4 text-green-600" />
                      <span className="text-green-700">Computer use capabilities available</span>
                    </div>
                  )}
                </div>
              )}
            </div>
          </CardContent>
        </Card>

        {settings.activeProvider && settings.providerSettings && (
          <Card>
            <CardHeader>
              <CardTitle>Provider Configuration</CardTitle>
              <CardDescription>
                Configure settings for {settings.activeProvider}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="api-key">API Key</Label>
                <Input
                  id="api-key"
                  type="password"
                  value={settings.formData.apiKey}
                  onChange={(e) =>
                    settings.setFormData((prev) => ({
                      ...prev,
                      apiKey: e.target.value,
                    }))
                  }
                  placeholder="Enter your API key"
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="model">
                  Model
                  {currentProvider?.computer_use_supported && (
                    <span className="text-xs text-muted-foreground ml-2">
                      (🖥️ = Computer Use)
                    </span>
                  )}
                </Label>
                <Select
                  value={settings.formData.model}
                  onValueChange={(value) =>
                    settings.setFormData((prev) => ({ ...prev, model: value }))
                  }
                >
                  <SelectTrigger>
                    <SelectValue placeholder="Select model" />
                  </SelectTrigger>
                  <SelectContent>
                    {(() => {
                      if (currentProvider?.model_info) {
                        return (
                          <>
                            {/* Computer Use Models */}
                            {currentProvider.model_info.filter(model => model.supports_computer_use).length > 0 && (
                              <>
                                <div className="px-2 py-1 text-xs font-medium text-muted-foreground bg-blue-50 border-b">
                                  Computer Use Models
                                </div>
                                {currentProvider.model_info
                                  .filter(model => model.supports_computer_use)
                                  .map((model) => (
                                    <SelectItem key={model.id} value={model.id}>
                                      <div className="flex items-center gap-2">
                                        <span>🖥️</span>
                                        <span>{model.name}</span>
                                        {model.is_recommended && (
                                          <Badge variant="outline" className="text-xs bg-green-50 text-green-700 border-green-200">
                                            Recommended
                                          </Badge>
                                        )}
                                      </div>
                                    </SelectItem>
                                  ))}
                              </>
                            )}

                            {/* General Chat Models */}
                            {currentProvider.model_info.filter(model => !model.supports_computer_use).length > 0 && (
                              <>
                                <div className="px-2 py-1 text-xs font-medium text-muted-foreground bg-gray-50 border-b">
                                  General Chat Models
                                </div>
                                {currentProvider.model_info
                                  .filter(model => !model.supports_computer_use)
                                  .map((model) => (
                                    <SelectItem key={model.id} value={model.id}>
                                      <div className="flex items-center gap-2">
                                        <span>💬</span>
                                        <span>{model.name}</span>
                                        {model.is_recommended && (
                                          <Badge variant="outline" className="text-xs bg-green-50 text-green-700 border-green-200">
                                            Recommended
                                          </Badge>
                                        )}
                                      </div>
                                    </SelectItem>
                                  ))}
                              </>
                            )}
                          </>
                        );
                      } else {
                        // Fallback to old model list format
                        return currentProvider?.models?.map((model) => (
                          <SelectItem key={model} value={model}>
                            {model}
                          </SelectItem>
                        )) || null;
                      }
                    })()}
                  </SelectContent>
                </Select>
                {(() => {
                  if (settings.formData.model && currentProvider?.model_info) {
                    const selectedModel = currentProvider.model_info.find(m => m.id === settings.formData.model);
                    if (selectedModel?.supports_computer_use) {
                      return (
                        <div className="text-xs text-muted-foreground">
                          ✅ This model supports computer use automation
                        </div>
                      );
                    } else if (selectedModel) {
                      return (
                        <div className="text-xs text-muted-foreground">
                          ⚠️ This model is for general chat only
                        </div>
                      );
                    }
                  }
                  return null;
                })()}
              </div>

              <div className="grid grid-cols-2 gap-4">
                <div className="space-y-2">
                  <Label htmlFor="max-tokens">Max Tokens</Label>
                  <Input
                    id="max-tokens"
                    type="number"
                    value={settings.formData.maxTokens}
                    onChange={(e) =>
                      settings.setFormData((prev) => ({
                        ...prev,
                        maxTokens: e.target.value,
                      }))
                    }
                    placeholder="e.g., 4000"
                  />
                </div>

                <div className="space-y-2">
                  <Label htmlFor="temperature">Temperature</Label>
                  <Input
                    id="temperature"
                    type="number"
                    step="0.1"
                    min="0"
                    max="2"
                    value={settings.formData.temperature}
                    onChange={(e) =>
                      settings.setFormData((prev) => ({
                        ...prev,
                        temperature: e.target.value,
                      }))
                    }
                    placeholder="e.g., 0.7"
                  />
                </div>
              </div>

              <div className="space-y-2">
                <Label htmlFor="system-prompt">System Prompt</Label>
                <Textarea
                  id="system-prompt"
                  value={settings.formData.systemPrompt}
                  onChange={(e) =>
                    settings.setFormData((prev) => ({
                      ...prev,
                      systemPrompt: e.target.value,
                    }))
                  }
                  placeholder="Enter custom system prompt (optional)"
                  rows={4}
                />
              </div>

              <Button
                onClick={settings.handleSaveProviderSettings}
                className="w-full"
              >
                <Save className="w-4 h-4 mr-2" />
                Save Provider Settings
              </Button>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  );
}