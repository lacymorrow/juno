# Error Handling Improvement Guide for DotDot

## Executive Summary

This audit reveals inconsistent error handling patterns across the DotDot codebase. While there's a comprehensive error handling module in Rust (`error_handling.rs`), its patterns aren't consistently applied throughout the codebase. TypeScript error handling is minimal, and there's a lack of React error boundaries.

## Current State Analysis

### Rust Error Handling

#### Strengths
1. **Comprehensive Error Module** (`src-tauri/src/error_handling.rs`)
   - Well-defined `JunoError` enum with categorized error types
   - Utility functions for consistent error formatting
   - Safe lock wrappers to prevent panics
   - Error recovery mechanisms for voice and agent systems

2. **Good Patterns Found**
   - Use of `Result<T, String>` in most command functions
   - Error logging with `tracing` crate
   - Error emission to frontend via events

#### Weaknesses
1. **Inconsistent Error Types**
   - Many functions use `Result<T, String>` instead of `Result<T, JunoError>`
   - String errors lose semantic information
   - No consistent error conversion patterns

2. **Unsafe Operations**
   - Direct use of `unwrap()` and `expect()` in some places
   - Missing error handling in async operations
   - Silent error suppression with `let _ =` patterns

3. **Missing Context**
   - Errors often lack contextual information
   - No error chaining or cause tracking
   - Limited use of error wrapping

### TypeScript Error Handling

#### Strengths
1. **Basic Error Utilities** (`src/lib/error-handling.ts`)
   - Type guard for error detection
   - Error conversion utilities
   - Basic error handling wrapper

2. **Hook Error Handling**
   - `useInvoke` hook includes try-catch with toast notifications
   - Some async operations have basic error handling

#### Weaknesses
1. **No Error Boundaries**
   - Missing React error boundaries for UI crash protection
   - No fallback UI for error states
   - Component crashes can bring down the entire app

2. **Inconsistent Patterns**
   - Mix of try-catch, `.catch()`, and unhandled promises
   - No standardized error logging
   - Missing error context in many places

3. **Silent Failures**
   - Many async operations don't handle errors
   - Console errors without user notification
   - No error recovery mechanisms

## Recommended Patterns

### 1. Rust Error Handling Standards

```rust
// Use custom error types with thiserror
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DotDotError {
    #[error("Permission denied: {0}")]
    Permission(String),
    
    #[error("Voice processing failed: {0}")]
    Voice(#[from] VoiceError),
    
    #[error("Agent execution failed: {0}")]
    Agent(#[from] AgentError),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

// Convert to frontend-friendly format
impl DotDotError {
    pub fn to_frontend(&self) -> FrontendError {
        FrontendError {
            code: self.error_code(),
            message: self.to_string(),
            details: self.details(),
            recoverable: self.is_recoverable(),
        }
    }
}

// Use Result type alias
type DotDotResult<T> = Result<T, DotDotError>;

// Add context with anyhow
use anyhow::{Context, Result};

pub async fn process_command(cmd: Command) -> Result<Response> {
    let data = load_data()
        .context("Failed to load command data")?;
    
    let result = execute_command(cmd, data)
        .await
        .context("Command execution failed")?;
    
    Ok(result)
}
```

### 2. TypeScript Error Handling Standards

```typescript
// Define error types
export class DotDotError extends Error {
  constructor(
    message: string,
    public code: string,
    public details?: unknown,
    public recoverable: boolean = true
  ) {
    super(message);
    this.name = 'DotDotError';
  }
}

// Error boundary component
import { Component, ErrorInfo, ReactNode } from 'react';

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  hasError: boolean;
  error?: Error;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('Error caught by boundary:', error, errorInfo);
    // Send to error reporting service
    reportError(error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return this.props.fallback || <ErrorFallback error={this.state.error} />;
    }

    return this.props.children;
  }
}

// Async error handler
export async function withErrorHandling<T>(
  operation: () => Promise<T>,
  options: {
    context?: string;
    fallback?: T;
    notify?: boolean;
    retry?: number;
  } = {}
): Promise<T> {
  const { context, fallback, notify = true, retry = 0 } = options;
  
  for (let attempt = 0; attempt <= retry; attempt++) {
    try {
      return await operation();
    } catch (error) {
      const dotError = toDotDotError(error, context);
      
      if (attempt < retry && dotError.recoverable) {
        await sleep(Math.pow(2, attempt) * 1000); // Exponential backoff
        continue;
      }
      
      logError(dotError);
      
      if (notify) {
        notifyError(dotError);
      }
      
      if (fallback !== undefined) {
        return fallback;
      }
      
      throw dotError;
    }
  }
  
  throw new Error('Unreachable');
}
```

### 3. Error Recovery Patterns

```rust
// Rust recovery pattern
pub async fn with_recovery<T, F, R>(
    operation: F,
    recovery: R,
) -> Result<T, DotDotError>
where
    F: Future<Output = Result<T, DotDotError>>,
    R: FnOnce(DotDotError) -> Future<Output = Result<T, DotDotError>>,
{
    match operation.await {
        Ok(result) => Ok(result),
        Err(error) => {
            warn!("Operation failed, attempting recovery: {}", error);
            recovery(error).await
        }
    }
}

// TypeScript recovery pattern
export async function withRecovery<T>(
  operation: () => Promise<T>,
  recovery: (error: DotDotError) => Promise<T>,
  options: { maxAttempts?: number } = {}
): Promise<T> {
  const { maxAttempts = 1 } = options;
  
  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    try {
      return await operation();
    } catch (error) {
      const dotError = toDotDotError(error);
      
      if (!dotError.recoverable || attempt === maxAttempts - 1) {
        throw dotError;
      }
      
      try {
        return await recovery(dotError);
      } catch (recoveryError) {
        throw toDotDotError(recoveryError, 'Recovery failed');
      }
    }
  }
  
  throw new Error('Unreachable');
}
```

## Implementation Checklist

### Phase 1: Foundation (Week 1)
- [ ] Create `DotDotError` type in Rust with thiserror
- [ ] Create `DotDotError` class in TypeScript
- [ ] Add React ErrorBoundary component
- [ ] Set up error reporting service integration

### Phase 2: Rust Migration (Week 2-3)
- [ ] Replace `Result<T, String>` with `Result<T, DotDotError>`
- [ ] Add context to all error points with anyhow
- [ ] Remove all `unwrap()` calls except in tests
- [ ] Implement recovery mechanisms for critical systems

### Phase 3: TypeScript Migration (Week 3-4)
- [ ] Wrap all async operations with error handling
- [ ] Add ErrorBoundary to all major UI sections
- [ ] Implement retry logic for network operations
- [ ] Create error notification system

### Phase 4: Testing & Monitoring (Week 5)
- [ ] Add error injection tests
- [ ] Test error recovery mechanisms
- [ ] Set up error monitoring dashboard
- [ ] Document error handling patterns

## Critical Areas Requiring Immediate Attention

1. **Agent System**: Multiple `unwrap()` calls that could panic
2. **Voice System**: Silent failures in transcription
3. **UI Components**: No error boundaries, crashes affect entire app
4. **Network Operations**: No retry logic or timeout handling
5. **File Operations**: Missing permission checks and recovery

## Metrics for Success

- Zero panics in production
- 90% of errors handled gracefully with user notification
- All async operations have timeout and cancellation
- Error recovery success rate > 80%
- Mean time to error resolution < 5 seconds

## Resources

- [Rust Error Handling Best Practices](https://blog.burntsushi.net/rust-error-handling/)
- [React Error Boundaries](https://react.dev/reference/react/Component#catching-rendering-errors-with-an-error-boundary)
- [TypeScript Error Handling Patterns](https://www.typescriptlang.org/docs/handbook/2/narrowing.html)
- [Error Monitoring with Sentry](https://docs.sentry.io/)