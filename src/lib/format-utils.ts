/**
 * Formatting and utility functions
 *
 * These functions are separate from constants to avoid being overwritten
 * when constants are regenerated from Rust.
 */

import { API_ENDPOINTS, PORTS, CHROME_DEBUG } from "./constants.generated";

/**
 * Format timeout duration in human-readable format
 */
export const formatTimeout = (ms: number): string => {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60000).toFixed(1)}m`;
};

/**
 * Extract file extension from filename
 */
export const getFileExtension = (filename: string): string => {
  const lastDot = filename.lastIndexOf(".");
  return lastDot === -1 ? "" : filename.substring(lastDot);
};

/**
 * Get development server URL
 */
export const getDevServerUrl = (): string => {
  return `${API_ENDPOINTS.ENDPOINTS_LOCALHOST_BASE}:${PORTS.VITE_DEV_PORT}`;
};

/**
 * Get Chrome debug URLs for different ports
 */
export const getChromeDebugUrls = (): string[] => [
  `${API_ENDPOINTS.ENDPOINTS_LOCALHOST_BASE}:${CHROME_DEBUG.PRIMARY}`,
  `${API_ENDPOINTS.ENDPOINTS_LOCALHOST_BASE}:${CHROME_DEBUG.ALT1}`,
  `${API_ENDPOINTS.ENDPOINTS_LOCALHOST_BASE}:${CHROME_DEBUG.ALT2}`,
];

/**
 * Format file size in human-readable format
 */
export const formatFileSize = (bytes: number): string => {
  if (bytes === 0) return "0 Bytes";

  const k = 1024;
  const sizes = ["Bytes", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));

  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
};

/**
 * Truncate string to specified length with ellipsis
 */
export const truncateString = (str: string, maxLength: number): string => {
  if (str.length <= maxLength) return str;
  return str.substring(0, maxLength - 3) + "...";
};

/**
 * Capitalize first letter of string
 */
export const capitalize = (str: string): string => {
  if (!str) return str;
  return str.charAt(0).toUpperCase() + str.slice(1);
};
