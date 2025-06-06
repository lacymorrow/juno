import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"
import { invoke } from "@tauri-apps/api/core"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

// Utility function to wrap Tauri's invoke with consistent error handling
export async function invokeCommand<T = any>(
  command: string,
  args?: Record<string, any>,
  operationName?: string // Optional third parameter for loading state tracking
): Promise<T> {
  try {
    const result = await invoke<T>(command, args);
    if (operationName) {
      console.log(`Operation ${operationName} completed successfully`);
    }
    return result;
  } catch (error) {
    console.error(`Failed to invoke command ${command}:`, error);
    if (operationName) {
      console.error(`Operation ${operationName} failed:`, error);
    }
    throw error;
  }
}
