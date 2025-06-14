import { useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';

interface UseInvokeOptions {
  showSuccessToast?: boolean;
  showErrorToast?: boolean;
  successMessage?: string;
  errorMessage?: string;
}

export function useInvoke() {
  const invokeCommand = useCallback(async <T = any>(
    command: string,
    args?: Record<string, any>,
    options?: UseInvokeOptions
  ): Promise<T> => {
    try {
      const result = await invoke<T>(command, args);
      
      if (options?.showSuccessToast && options?.successMessage) {
        toast.success(options.successMessage);
      }
      
      return result;
    } catch (error) {
      console.error(`Failed to invoke command ${command}:`, error);
      
      if (options?.showErrorToast !== false) {
        const errorMsg = options?.errorMessage || `Failed to execute ${command}`;
        toast.error(errorMsg);
      }
      
      throw error;
    }
  }, []);

  return { invokeCommand };
}