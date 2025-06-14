import { useCallback, useState } from 'react';

export interface StateManager<T> {
  value: T;
  setValue: (value: T) => void;
  updateValue: (updater: (prev: T) => T) => void;
  resetValue: () => void;
}

export function useStateManager<T>(initialValue: T): StateManager<T> {
  const [value, setValue] = useState<T>(initialValue);

  const updateValue = useCallback((updater: (prev: T) => T) => {
    setValue(updater);
  }, []);

  const resetValue = useCallback(() => {
    setValue(initialValue);
  }, [initialValue]);

  return {
    value,
    setValue,
    updateValue,
    resetValue,
  };
}

// Utility for managing multiple related state values
export function useMultiStateManager<T extends Record<string, any>>(initialState: T) {
  const [state, setState] = useState<T>(initialState);

  const updateField = useCallback(<K extends keyof T>(field: K, value: T[K]) => {
    setState(prev => ({ ...prev, [field]: value }));
  }, []);

  const updateFields = useCallback((updates: Partial<T>) => {
    setState(prev => ({ ...prev, ...updates }));
  }, []);

  const resetState = useCallback(() => {
    setState(initialState);
  }, [initialState]);

  return {
    state,
    setState,
    updateField,
    updateFields,
    resetState,
  };
}