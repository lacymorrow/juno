/**
 * Validation utilities
 *
 * These functions are separate from constants to avoid being overwritten
 * when constants are regenerated from Rust.
 */

import { REGEX_PATTERNS } from './constants.generated';

/**
 * Validate email address format
 */
export const validateEmail = (email: string): boolean => REGEX_PATTERNS.EMAIL.test(email);

/**
 * Validate URL format
 */
export const validateUrl = (url: string): boolean => REGEX_PATTERNS.URL.test(url);

/**
 * Validate wake word format
 */
export const validateWakeWord = (word: string): boolean => REGEX_PATTERNS.WAKE_WORD.test(word);

/**
 * Validate JSON string format
 */
export const validateJson = (text: string): boolean => REGEX_PATTERNS.JSON.test(text);

/**
 * Check if string is only whitespace
 */
export const isWhitespaceOnly = (text: string): boolean => REGEX_PATTERNS.WHITESPACE_ONLY.test(text);

/**
 * Check if string starts with command prefix
 */
export const hasCommandPrefix = (text: string): boolean => REGEX_PATTERNS.COMMAND_PREFIX.test(text);
