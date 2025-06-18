import { Badge } from "@/components/ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useSettings } from "@/hooks/useSettings";
import { Brain, CheckCircle, AlertCircle } from "lucide-react";

interface ProviderSelectorProps {
  variant?: "compact" | "full";
  className?: string;
}

export function ProviderSelector({
  variant = "compact",
  className = "",
}: ProviderSelectorProps) {
  const settings = useSettings();

  const currentProvider = settings.providers.find(
    (p) => p.id === settings.activeProvider
  );

  if (settings.isLoading) {
    return (
      <div className={`flex items-center gap-2 ${className}`}>
        <div className="w-4 h-4 bg-muted animate-pulse rounded" />
        <div className="w-20 h-4 bg-muted animate-pulse rounded" />
      </div>
    );
  }

  return (
    <div className={`flex items-center gap-2 ${className}`}>
      {variant === "full" && (
        <div className="flex items-center gap-1">
          <Brain size={14} className="text-muted-foreground" />
          <span className="text-xs text-muted-foreground">Provider:</span>
        </div>
      )}

      <Select
        value={settings.activeProvider}
        onValueChange={settings.handleActiveProviderChange}
      >
        <SelectTrigger
          className={
            variant === "compact"
              ? "h-7 text-xs border-none bg-transparent hover:bg-muted/50 w-auto"
              : "h-8"
          }
        >
          <SelectValue placeholder="Select provider">
            {currentProvider && (
              <div className="flex items-center gap-1">
                <div className="flex items-center gap-1">
                  {currentProvider.is_available ? (
                    <CheckCircle size={10} className="text-green-500" />
                  ) : (
                    <AlertCircle size={10} className="text-red-500" />
                  )}
                </div>
                <span className="text-xs font-medium">
                  {variant === "compact"
                    ? currentProvider.name.replace(
                        /\s+(Claude|GPT|Gemini)/i,
                        ""
                      )
                    : currentProvider.name}
                </span>
              </div>
            )}
          </SelectValue>
        </SelectTrigger>
        <SelectContent>
          {settings.providers.map((provider) => (
            <SelectItem key={provider.id} value={provider.id}>
              <div className="flex items-center gap-2">
                <div className="flex items-center gap-1">
                  {provider.is_available ? (
                    <CheckCircle size={12} className="text-green-500" />
                  ) : (
                    <AlertCircle size={12} className="text-red-500" />
                  )}
                </div>
                <div className="flex flex-col">
                  <div className="flex items-center gap-2">
                    <span className="font-medium">{provider.name}</span>
                    {provider.computer_use_supported && (
                      <Badge
                        variant="outline"
                        className="text-xs bg-blue-50 text-blue-700 border-blue-200"
                      >
                        Computer Use
                      </Badge>
                    )}
                    {provider.is_default && (
                      <Badge
                        variant="outline"
                        className="text-xs bg-green-50 text-green-700 border-green-200"
                      >
                        Active
                      </Badge>
                    )}
                  </div>
                  {variant === "full" && (
                    <span className="text-xs text-muted-foreground">
                      {provider.description}
                    </span>
                  )}
                </div>
              </div>
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      {variant === "full" && currentProvider && (
        <div className="text-xs text-muted-foreground">
          <Badge
            variant="outline"
            className={`text-xs ${
              currentProvider.is_available
                ? "bg-green-50 text-green-700 border-green-200"
                : "bg-red-50 text-red-700 border-red-200"
            }`}
          >
            {currentProvider.is_available ? "Available" : "API Key Required"}
          </Badge>
        </div>
      )}
    </div>
  );
}
