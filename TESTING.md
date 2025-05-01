# Testing in Juno

This document describes the testing approach for the Juno project.

## Frontend Tests

The frontend tests use Vitest with JSDOM for testing React components and utility functions.

### Running Tests

To run the frontend tests:

```bash
npm test
# or
npx vitest run
```

To run tests in watch mode:

```bash
npm run test:watch
# or
npx vitest
```

### Test Structure

- Tests are located next to the files they test with a `.test.ts` or `.test.tsx` extension
- The test setup is in `src/test/setup.ts`
- Vitest configuration is in `vitest.config.ts`

### Current Test Coverage

- `ttsService.ts`: Tests for speech synthesis functionality
  - Tests for handling empty text
  - Tests for local speech synthesis
  - Tests for API-based speech synthesis
  - Tests for error handling (offline, API errors, playback errors)

## Backend Tests

The backend tests use Rust's built-in testing framework.

### Running Tests

To run the backend tests:

```bash
cd src-tauri
cargo test
```

### Test Structure

- Tests are located in the same files as the code they test, or in a `tests.rs` file in the same module
- Tests are annotated with `#[test]` and are in modules annotated with `#[cfg(test)]`

### Current Test Coverage

- `tools/helpers.rs`: Tests for parameter extraction and key handling
  - Tests for string parameter extraction
  - Tests for numeric parameter extraction
  - Tests for optional parameter extraction
  - Tests for the `hold_keys_and_run` function
  - Tests for the `str_replace_editor` function

## Future Test Improvements

- Add more tests for React components
- Add integration tests for the Tauri app
- Set up CI/CD to run tests automatically
- Add code coverage reporting