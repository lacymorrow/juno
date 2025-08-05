# Rate Limiting Configuration Guide

## Overview

Juno implements a comprehensive rate limiting system to protect against abuse and ensure system stability. This guide covers configuration, customization, and best practices.

## Current Implementation

### Default Rate Limits

The following rate limits are currently hardcoded in `src-tauri/src/utils/rate_limiter.rs`:

```rust
pub struct GlobalRateLimiters {
    pub ai_operations: RateLimiter,      // 20/minute
    pub file_operations: RateLimiter,    // 100/second
    pub shell_commands: RateLimiter,     // 10/second
    pub screenshots: RateLimiter,        // 5/second
    pub browser_operations: RateLimiter, // 30/minute
}
```

### Rate Limit Categories

#### 1. AI Operations (20/minute)
**Protected Commands:**
- `submit_query`
- `submit_orchestrated_query`
- Other AI-powered operations

**Rationale:** AI API calls are expensive and should be limited to prevent cost overruns.

#### 2. Shell Commands (10/second)
**Protected Commands:**
- `bash_command`
- Any command execution operations

**Rationale:** Shell commands pose security risks and should be strictly limited.

#### 3. Screenshots (5/second)
**Protected Commands:**
- `capture_screenshot_command`
- `computer` command with screenshot action

**Rationale:** Screenshots are resource-intensive and can impact system performance.

#### 4. File Operations (100/second)
**Protected Commands:**
- File read/write operations
- Directory operations
- Keyboard operations (temporarily using this limiter)

**Rationale:** Prevents filesystem abuse while allowing reasonable usage.

#### 5. Browser Operations (30/minute)
**Protected Commands:**
- Browser automation
- Web scraping operations

**Rationale:** Prevents abuse of web resources and respects website rate limits.

## How Rate Limiting Works

### Token Bucket Algorithm

Each rate limiter uses a token bucket algorithm:

1. **Bucket Capacity**: Maximum number of tokens (requests) allowed
2. **Refill Rate**: How quickly tokens are replenished
3. **Token Consumption**: Each request consumes one or more tokens
4. **Burst Support**: Can handle occasional spikes up to bucket capacity

### Example Flow

```rust
// When a command is called:
if let Err(e) = state.rate_limiters.ai_operations.check("user_id").await {
    // Rate limit exceeded
    return Err(e.to_user_message()); // User-friendly error
}
// Proceed with operation
```

### User Identification

Currently, all rate limits use a static `"default_user"` key. This should be enhanced to support:
- Per-session identification
- Per-API-key tracking
- Per-window context

## Configuration (Future)

### Planned Configuration File

```json
// ~/.juno/rate_limits.json
{
  "ai_operations": {
    "per_minute": 20,
    "burst_capacity": 5,
    "strict_mode": false
  },
  "shell_commands": {
    "per_second": 10,
    "burst_capacity": 2,
    "strict_mode": true
  },
  "screenshots": {
    "per_second": 5,
    "burst_capacity": 3,
    "strict_mode": false
  },
  "file_operations": {
    "per_second": 100,
    "burst_capacity": 20,
    "strict_mode": false
  },
  "browser_operations": {
    "per_minute": 30,
    "burst_capacity": 10,
    "strict_mode": false
  }
}
```

### Environment Variables (Future)

```bash
JUNO_RATE_LIMIT_AI_PER_MINUTE=20
JUNO_RATE_LIMIT_SHELL_PER_SECOND=10
JUNO_RATE_LIMIT_SCREENSHOTS_PER_SECOND=5
JUNO_RATE_LIMIT_FILES_PER_SECOND=100
JUNO_RATE_LIMIT_BROWSER_PER_MINUTE=30
```

## Implementing Rate Limiting in New Commands

### Step 1: Identify Category

Determine which rate limiter category your command falls into:
- Expensive API calls → `ai_operations`
- System commands → `shell_commands`
- Visual operations → `screenshots`
- File system access → `file_operations`
- Web automation → `browser_operations`

### Step 2: Add Rate Limit Check

```rust
#[tauri::command]
pub async fn your_command(
    state: State<'_, AppState>
) -> Result<String, String> {
    // Add rate limit check at the beginning
    if let Err(e) = state.rate_limiters.appropriate_limiter.check("user_id").await {
        warn!("Rate limit exceeded for your_command");
        return Err(e.to_user_message());
    }
    
    // Your command logic here
    Ok("Success".to_string())
}
```

### Step 3: Use the Macro (Optional)

```rust
#[tauri::command]
pub async fn your_command(
    state: State<'_, AppState>
) -> Result<String, String> {
    // Use the rate_limit_check! macro
    rate_limit_check!(state.rate_limiters.appropriate_limiter, "user_id");
    
    // Your command logic here
    Ok("Success".to_string())
}
```

## Error Handling

### Rate Limit Error Response

When rate limited, commands return user-friendly errors:

```json
{
  "error": "Rate limit exceeded for AI operations. Please retry after 30 seconds."
}
```

### Client-Side Handling

```typescript
try {
  const result = await invoke('submit_query', { query });
  // Handle success
} catch (error) {
  if (error.includes('Rate limit exceeded')) {
    // Extract retry-after time
    const retryAfter = parseInt(error.match(/retry after (\d+) seconds/)?.[1] || '60');
    // Show user-friendly message
    showNotification(`Please wait ${retryAfter} seconds before trying again`);
  }
}
```

## Monitoring and Debugging

### Debug Logging

Enable debug logs to see rate limiting in action:

```bash
RUST_LOG=debug bun run tauri dev
```

### Log Output Example

```
[DEBUG] Rate limit check passed for key: default_user (bucket: ai_operations)
[WARN] Rate limit exceeded for AI operations
[DEBUG] Completed rate limiter cleanup
```

## Best Practices

### 1. Choose Appropriate Limits
- Consider operation cost and resource usage
- Allow reasonable burst capacity
- Be more restrictive in production

### 2. User Feedback
- Always provide clear error messages
- Include retry-after information
- Consider showing remaining quota

### 3. Graceful Degradation
- Queue non-critical operations
- Offer alternatives when rate limited
- Cache results when possible

### 4. Security Considerations
- Never bypass rate limits for "trusted" users
- Log rate limit violations for security monitoring
- Consider IP-based limits for additional protection

## Future Enhancements

### 1. Persistent State
Store rate limit state in database to persist across restarts:
```rust
// Save bucket state periodically
rate_limiter.save_state_to_db().await;
```

### 2. Distributed Rate Limiting
For multi-instance deployments:
```rust
// Use Redis or similar for shared state
let distributed_limiter = DistributedRateLimiter::new(redis_client);
```

### 3. Dynamic Limits
Adjust limits based on system load:
```rust
// Reduce limits when system is under stress
if system_load > 0.8 {
    rate_limiter.reduce_limits_by(0.5);
}
```

### 4. User Quotas
Implement per-user quotas with different tiers:
```rust
let user_tier = get_user_tier(user_id);
let limits = match user_tier {
    Tier::Free => RateLimits::free(),
    Tier::Pro => RateLimits::pro(),
    Tier::Enterprise => RateLimits::enterprise(),
};
```

## Troubleshooting

### Common Issues

1. **"No reactor running" panic**
   - Ensure rate limiter is initialized after Tokio runtime
   - Check that `initialize_rate_limiter_cleanup()` is called in async context

2. **Rate limits too restrictive**
   - Temporarily increase limits for development
   - Consider implementing bypass for local testing

3. **Cleanup not working**
   - Verify cleanup task is running (check logs)
   - Ensure proper tokio runtime initialization

### Testing Rate Limits

```bash
# Test script to trigger rate limits
for i in {1..25}; do
  curl -X POST http://localhost:1420/api/submit_query \
    -H "Content-Type: application/json" \
    -d '{"query": "test"}'
  echo "Request $i"
done
```

## Conclusion

Rate limiting is essential for maintaining system stability and preventing abuse. While the current implementation uses hardcoded limits, the architecture supports future enhancements for configuration, persistence, and distributed deployments.