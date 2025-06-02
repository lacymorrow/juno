# Juno - AI Computer Use Agent

Juno is a Tauri v2 application with Anthropic Computer Use Bot capabilities, built with React and TypeScript.

## Setup Instructions

### Prerequisites
- Node.js and Bun package manager
- Rust and Cargo
- Tauri CLI v2

### Installation

1. Clone the repository
2. Install dependencies: `bun install`
3. Copy `.env.example` to `.env` and add your API keys:
   ```bash
   cp .env.example .env
   ```
4. Edit `.env` with your actual API keys for:
   - OpenAI API
   - Anthropic API
   - Google Gemini API
   - ElevenLabs API
   - Perplexity API
   - HuggingFace API
   - Replicate API
   - FAL.ai API

### Development
- Run in development: `bun run tauri dev`
- Build for production: `bun run tauri build`

### Testing
- Run tests: `bun run test`
- Run Rust tests: `./test-rust-units.sh`

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Security Notice

Never commit your `.env` file containing real API keys. Always use the `.env.example` template for sharing configuration.
