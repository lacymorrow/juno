import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

interface ErrorToastAction {
  label: string;
  command: string;
  args: any;
}

interface ErrorToastPayload {
  message: string;
  action?: ErrorToastAction;
}

export function ErrorToast() {
  const [error, setError] = useState<ErrorToastPayload | null>(null);

  useEffect(() => {
    const unlisten = listen<ErrorToastPayload>('show-error-toast', (event) => {
      setError(event.payload);
      setTimeout(() => setError(null), 5000); // Auto-hide after 5 seconds
    });

    return () => {
      unlisten.then(f => f());
    };
  }, []);

  if (!error) {
    return null;
  }

  const handleAction = () => {
    if (error.action) {
      invoke(error.action.command, error.action.args).catch(console.error);
    }
    setError(null);
  };

  return (
    <div className="fixed bottom-4 right-4 bg-red-500 text-white p-4 rounded-lg shadow-lg max-w-sm">
      <p>{error.message}</p>
      {error.action && (
        <button
          onClick={handleAction}
          className="mt-2 bg-white text-red-500 px-2 py-1 rounded"
        >
          {error.action.label}
        </button>
      )}
    </div>
  );
}
