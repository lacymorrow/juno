import { ReactNode, useEffect } from 'react';
import { ErrorBoundary } from './ErrorBoundary';

interface AsyncErrorBoundaryProps {
  children: ReactNode;
  fallback?: ReactNode;
  onError?: (error: Error) => void;
}

/**
 * Error boundary that also catches async errors and unhandled promise rejections
 * within its component tree.
 */
export function AsyncErrorBoundary({ 
  children, 
  fallback, 
  onError 
}: AsyncErrorBoundaryProps) {
  useEffect(() => {
    // Handle unhandled promise rejections
    const handleUnhandledRejection = (event: PromiseRejectionEvent) => {
      // Enhanced error logging for debugging
      console.error('🔴 Unhandled Promise Rejection Details:');
      console.error('Reason:', event.reason);
      console.error('Promise:', event.promise);
      
      // Try to extract more information
      if (event.reason instanceof Error) {
        console.error('Error Stack:', event.reason.stack);
        console.error('Error Message:', event.reason.message);
      }
      
      // Log the specific error we're seeing
      if (typeof event.reason === 'function' && event.reason.toString().includes('unlisten')) {
        console.error('🎯 FOUND IT: Unlisten function rejection!');
        console.error('Function:', event.reason.toString());
        console.trace('Stack trace at rejection point');
      }
      
      // Check if this is a Tauri event cleanup error
      if (event.reason && 
          (event.reason.toString().includes('__TAURI_EVENT_PLUGIN_INTERNALS__') ||
           event.reason.toString().includes('unregisterListener') ||
           (event.reason instanceof TypeError && event.reason.message.includes('undefined is not an object')))) {
        console.warn('🔧 Tauri event cleanup error (safe to ignore):', event.reason);
        // Prevent the error from propagating - these are expected during cleanup
        event.preventDefault();
        return;
      }
      
      const error = new Error(
        `Unhandled promise rejection: ${event.reason || 'Unknown error'}`
      );

      if (onError) {
        onError(error);
      }
      
      // Re-throw to be caught by error boundary
      throw error;
    };

    window.addEventListener('unhandledrejection', handleUnhandledRejection);

    return () => {
      window.removeEventListener('unhandledrejection', handleUnhandledRejection);
    };
  }, [onError]);

  return (
    <ErrorBoundary 
      fallback={fallback}
      onError={(error) => {
        if (onError) {
          onError(error);
        }
      }}
    >
      {children}
    </ErrorBoundary>
  );
}

// Component-specific error boundary for critical features
export function FeatureErrorBoundary({ 
  children, 
  featureName 
}: { 
  children: ReactNode; 
  featureName: string;
}) {
  return (
    <ErrorBoundary
      fallback={
        <div className="p-4 border border-destructive/20 rounded-md bg-destructive/5">
          <p className="text-sm text-destructive">
            The {featureName} feature encountered an error and has been disabled.
          </p>
          <p className="text-xs text-muted-foreground mt-1">
            Please try refreshing the page or contact support if the issue persists.
          </p>
        </div>
      }
      onError={(error) => {
        console.error(`Error in ${featureName}:`, error);
      }}
    >
      {children}
    </ErrorBoundary>
  );
}