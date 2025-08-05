# Changelog - Rate Limiting Implementation

## [Unreleased] - 2025-07-28

### Added
- **Rate Limiting System**: Comprehensive token bucket-based rate limiting for all operations
  - AI Operations: 20 requests per minute
  - Shell Commands: 10 requests per second  
  - Screenshots: 5 requests per second
  - File Operations: 100 requests per second
  - Browser Operations: 30 requests per minute
- **Rate Limiter Features**:
  - Token bucket algorithm with automatic refill
  - Per-user tracking capability (currently using default user)
  - Automatic cleanup of stale buckets every 5 minutes
  - User-friendly error messages with retry-after information
  - Burst capacity support for legitimate usage spikes

### Fixed
- **Critical Tokio Runtime Bug**: Fixed panic when rate limiter cleanup task was initialized before Tokio runtime
  - Deferred cleanup task initialization to after runtime is ready
  - Added `initialize_rate_limiter_cleanup()` method to AppState
- **BrowserController Drop Panic**: Fixed panic when BrowserController dropped outside Tokio runtime
  - Added runtime existence check using `tokio::runtime::Handle::try_current()`
  - Implemented synchronous fallback for temp profile cleanup
- **Memory Safety**: Eliminated potential panics from unsafe operations
  - Fixed compilation error in capture_screenshot_command calls
  - Added proper state parameter passing

### Changed
- **AppState Initialization**: Rate limiter cleanup now initialized in lib.rs setup() function
- **Drop Implementation Pattern**: Established safe pattern for async cleanup in Drop traits

### Security
- Rate limiting prevents abuse of expensive operations (AI API calls)
- Protection against shell command injection attacks
- Resource exhaustion prevention for screenshots and browser automation
- Configurable limits for different security contexts (dev vs prod)

### Technical Details
- Implementation in `src-tauri/src/utils/rate_limiter.rs`
- Integration points in all major command handlers
- Thread-safe implementation using Arc<Mutex<HashMap>>
- Efficient O(1) token consumption checks

### Documentation
- Updated CLAUDE.md with rate limiting section and Tokio runtime safety guidelines
- Updated README.md with rate limiting overview
- Updated API.md with rate limit specifications
- Created TOKIO_RUNTIME_BUGS_REPORT.md documenting all runtime issues found and fixed

### Future Enhancements
- Configuration via settings.json
- Per-user rate limit overrides  
- Environment-specific limits (development vs production)
- Distributed rate limiting for multi-instance deployments
- Persistent rate limit state across restarts
- Rate limit metrics and monitoring