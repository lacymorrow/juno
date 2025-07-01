/// <reference types="node" />

// Temporary declaration so TS doesn't error when node types are unavailable
// Remove once @types/node is added to the project
// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore
declare const process: { env: Record<string, string | undefined> };

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import path from "path";
import { defineConfig, type UserConfig } from "vite";
import { PORTS } from "./src/lib/constants.generated";

const host: string | undefined = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async (): Promise<UserConfig> => ({
	plugins: [react(), tailwindcss()],
	resolve: {
		alias: {
			"@": path.resolve(__dirname, "./src"),
		},
	},

	// Strip console.* and debugger statements from production builds
	build: {
		minify: "terser",
		terserOptions: {
			compress: {
				drop_console: true,
				drop_debugger: true,
			},
		},
	},

	// Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
	//
	// 1. prevent vite from obscuring rust errors
	clearScreen: false,
	// 2. tauri expects a fixed port, fail if that port is not available
	server: {
		port: PORTS.VITE_DEV_PORT,
		strictPort: true,
		host: host || false,
		hmr: host
			? {
				protocol: "ws",
				host,
				port: PORTS.VITE_HMR_PORT,
			}
			: undefined,
		watch: {
			// 3. tell vite to ignore watching `src-tauri`
			ignored: ["**/src-tauri/**"],
		},
	},
}));
