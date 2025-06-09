import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock Tauri API before importing the utils module
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { cn, invokeCommand } from '../utils';

describe('utils', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset console mocks
    vi.spyOn(console, 'log').mockImplementation(() => {});
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  describe('cn (className utility)', () => {
    it('merges class names correctly', () => {
      const result = cn('base-class', 'additional-class');
      expect(result).toBe('base-class additional-class');
    });

    it('handles conditional classes', () => {
      const result = cn('base-class', true && 'conditional-class', false && 'hidden-class');
      expect(result).toBe('base-class conditional-class');
    });

    it('merges Tailwind classes correctly', () => {
      const result = cn('p-4', 'p-2'); // Should prioritize p-2
      expect(result).toBe('p-2');
    });

    it('handles empty inputs', () => {
      const result = cn();
      expect(result).toBe('');
    });

    it('handles array inputs', () => {
      const result = cn(['class1', 'class2'], 'class3');
      expect(result).toBe('class1 class2 class3');
    });

    it('handles object inputs', () => {
      const result = cn({
        'active': true,
        'inactive': false,
        'default': true
      });
      expect(result).toBe('active default');
    });
  });

  describe('invokeCommand', () => {
    it('successfully invokes command and returns result', async () => {
      const mockInvoke = await import('@tauri-apps/api/core').then(m => m.invoke);
      const mockResult = { success: true, data: 'test' };
      vi.mocked(mockInvoke).mockResolvedValue(mockResult);

      const result = await invokeCommand('test-command', { arg1: 'value1' });

      expect(mockInvoke).toHaveBeenCalledWith('test-command', { arg1: 'value1' });
      expect(result).toEqual(mockResult);
    });

    it('logs success message when operationName is provided', async () => {
      const mockInvoke = await import('@tauri-apps/api/core').then(m => m.invoke);
      const mockResult = { success: true };
      vi.mocked(mockInvoke).mockResolvedValue(mockResult);
      const consoleSpy = vi.spyOn(console, 'log');

      await invokeCommand('test-command', {}, 'test-operation');

      expect(consoleSpy).toHaveBeenCalledWith('Operation test-operation completed successfully');
    });

    it('handles commands without arguments', async () => {
      const mockInvoke = await import('@tauri-apps/api/core').then(m => m.invoke);
      const mockResult = { data: 'no-args' };
      vi.mocked(mockInvoke).mockResolvedValue(mockResult);

      const result = await invokeCommand('simple-command');

      expect(mockInvoke).toHaveBeenCalledWith('simple-command', undefined);
      expect(result).toEqual(mockResult);
    });

    it('throws error when command fails', async () => {
      const mockInvoke = await import('@tauri-apps/api/core').then(m => m.invoke);
      const mockError = new Error('Command failed');
      vi.mocked(mockInvoke).mockRejectedValue(mockError);

      await expect(invokeCommand('failing-command')).rejects.toThrow('Command failed');
      expect(mockInvoke).toHaveBeenCalledWith('failing-command', undefined);
    });

    it('logs error messages when command fails', async () => {
      const mockInvoke = await import('@tauri-apps/api/core').then(m => m.invoke);
      const mockError = new Error('Command failed');
      vi.mocked(mockInvoke).mockRejectedValue(mockError);
      const consoleSpy = vi.spyOn(console, 'error');

      try {
        await invokeCommand('failing-command', { arg: 'value' }, 'failing-operation');
      } catch (error) {
        // Expected to throw
      }

      expect(consoleSpy).toHaveBeenCalledWith('Failed to invoke command failing-command:', mockError);
      expect(consoleSpy).toHaveBeenCalledWith('Operation failing-operation failed:', mockError);
    });

    it('logs error without operation name when not provided', async () => {
      const mockInvoke = await import('@tauri-apps/api/core').then(m => m.invoke);
      const mockError = new Error('Command failed');
      vi.mocked(mockInvoke).mockRejectedValue(mockError);
      const consoleSpy = vi.spyOn(console, 'error');

      try {
        await invokeCommand('failing-command');
      } catch (error) {
        // Expected to throw
      }

      expect(consoleSpy).toHaveBeenCalledWith('Failed to invoke command failing-command:', mockError);
      expect(consoleSpy).toHaveBeenCalledTimes(1);
    });

    it('works with typed return values', async () => {
      const mockInvoke = await import('@tauri-apps/api/core').then(m => m.invoke);
      interface TestResult {
        id: number;
        name: string;
      }

      const mockResult: TestResult = { id: 1, name: 'test' };
      vi.mocked(mockInvoke).mockResolvedValue(mockResult);

      const result = await invokeCommand<TestResult>('typed-command');

      expect(result).toEqual(mockResult);
      expect(result.id).toBe(1);
      expect(result.name).toBe('test');
    });
  });
});