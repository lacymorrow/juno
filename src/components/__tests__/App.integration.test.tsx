import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { a11yUtils, performanceUtils, testUtils } from "../../test/setupTests";
import App from "../App";

describe("App Integration Tests", () => {
  const user = userEvent.setup();

  beforeEach(() => {
    testUtils.resetMocks();
  });

  describe("User Workflows", () => {
    it("should handle complete voice interaction workflow", async () => {
      render(<App />);

      // Start listening
      const listenButton = screen.getByRole("button", {
        name: /start listening/i,
      });
      await user.click(listenButton);

      expect(window.__TAURI__.core.invoke).toHaveBeenCalledWith(
        "app:toggle_listening"
      );

      // Simulate voice input
      testUtils.mockUserVoiceInput("Take a screenshot");

      // Wait for agent response
      await waitFor(() => {
        expect(screen.getByText(/mock agent response/i)).toBeInTheDocument();
      });

      // Verify tool calls are displayed
      const toolCallSection = screen.getByTestId("tool-calls");
      expect(toolCallSection).toBeInTheDocument();
    });

    it("should handle settings workflow", async () => {
      render(<App />);

      // Open settings
      const settingsButton = screen.getByRole("button", { name: /settings/i });
      await user.click(settingsButton);

      // Navigate to voice settings
      const voiceTab = screen.getByRole("tab", { name: /voice/i });
      await user.click(voiceTab);

      // Change voice settings
      const wakeWordToggle = screen.getByRole("switch", { name: /wake word/i });
      await user.click(wakeWordToggle);

      // Save settings
      const saveButton = screen.getByRole("button", { name: /save/i });
      await user.click(saveButton);

      expect(window.__TAURI__.core.invoke).toHaveBeenCalledWith(
        "settings:update",
        expect.objectContaining({
          voiceSettings: expect.objectContaining({
            wakeWordEnabled: true,
          }),
        })
      );
    });

    it("should handle chat export/import workflow", async () => {
      render(<App />);

      // Open chat menu
      const menuButton = screen.getByRole("button", { name: /menu/i });
      await user.click(menuButton);

      // Export chat
      const exportButton = screen.getByRole("menuitem", { name: /export/i });
      await user.click(exportButton);

      expect(window.__TAURI__.core.invoke).toHaveBeenCalledWith("chat:export");

      // Import chat
      const importButton = screen.getByRole("menuitem", { name: /import/i });
      await user.click(importButton);

      // Simulate file selection
      const fileInput = screen.getByLabelText(/import chat file/i);
      const mockFile = new File(['{"messages": []}'], "chat.json", {
        type: "application/json",
      });
      await user.upload(fileInput, mockFile);

      expect(window.__TAURI__.core.invoke).toHaveBeenCalledWith(
        "chat:import",
        expect.objectContaining({
          data: expect.any(String),
        })
      );
    });

    it("should handle feedback submission workflow", async () => {
      render(<App />);

      // Open help modal
      const helpButton = screen.getByRole("button", { name: /help/i });
      await user.click(helpButton);

      // Open feedback form
      const feedbackButton = screen.getByRole("button", { name: /feedback/i });
      await user.click(feedbackButton);

      // Fill feedback form
      const titleInput = screen.getByLabelText(/title/i);
      await user.type(titleInput, "Test feedback");

      const descriptionInput = screen.getByLabelText(/description/i);
      await user.type(descriptionInput, "This is a test feedback message");

      const typeSelect = screen.getByLabelText(/type/i);
      await user.click(typeSelect);
      const bugOption = screen.getByRole("option", { name: /bug/i });
      await user.click(bugOption);

      const prioritySelect = screen.getByLabelText(/priority/i);
      await user.click(prioritySelect);
      const highOption = screen.getByRole("option", { name: /high/i });
      await user.click(highOption);

      // Submit feedback
      const submitButton = screen.getByRole("button", { name: /submit/i });
      await user.click(submitButton);

      expect(window.__TAURI__.core.invoke).toHaveBeenCalledWith(
        "feedback:submit",
        expect.objectContaining({
          title: "Test feedback",
          description: "This is a test feedback message",
          type: "bug",
          priority: "high",
        })
      );
    });

    it("should handle window management workflow", async () => {
      render(<App />);

      // Minimize window
      const minimizeButton = screen.getByRole("button", { name: /minimize/i });
      await user.click(minimizeButton);

      expect(window.__TAURI__.core.invoke).toHaveBeenCalledWith(
        "window:minimize"
      );

      // Maximize window
      const maximizeButton = screen.getByRole("button", { name: /maximize/i });
      await user.click(maximizeButton);

      expect(window.__TAURI__.core.invoke).toHaveBeenCalledWith(
        "window:maximize"
      );

      // Toggle fullscreen
      testUtils.mockKeyboardShortcut("f", ["cmd"]);

      await waitFor(() => {
        expect(window.__TAURI__.core.invoke).toHaveBeenCalledWith(
          "window:set_fullscreen",
          expect.objectContaining({ fullscreen: true })
        );
      });
    });
  });

  describe("Error Handling", () => {
    it("should handle API errors gracefully", async () => {
      // Mock API error
      testUtils.mockError("agent:submit_query", "API rate limit exceeded");

      render(<App />);

      // Trigger agent request
      const input = screen.getByRole("textbox", { name: /chat input/i });
      await user.type(input, "Test message");
      await user.keyboard("{Enter}");

      // Verify error is displayed
      await waitFor(() => {
        expect(screen.getByText(/error.*api rate limit/i)).toBeInTheDocument();
      });

      // Verify retry button is available
      const retryButton = screen.getByRole("button", { name: /retry/i });
      expect(retryButton).toBeInTheDocument();
    });

    it("should handle network errors gracefully", async () => {
      // Mock network error
      vi.mocked(fetch).mockRejectedValueOnce(new Error("Network error"));

      render(<App />);

      // Trigger network-dependent action (feedback submission)
      const helpButton = screen.getByRole("button", { name: /help/i });
      await user.click(helpButton);

      const feedbackButton = screen.getByRole("button", { name: /feedback/i });
      await user.click(feedbackButton);

      // Submit feedback (should trigger network request)
      const submitButton = screen.getByRole("button", { name: /submit/i });
      await user.click(submitButton);

      // Verify network error is handled
      await waitFor(() => {
        expect(screen.getByText(/network error/i)).toBeInTheDocument();
      });
    });

    it("should handle permission errors gracefully", async () => {
      // Mock permission denied
      testUtils.mockError(
        "permissions:request_accessibility",
        "Permission denied"
      );

      render(<App />);

      // Trigger permission request
      const permissionButton = screen.getByRole("button", {
        name: /grant permissions/i,
      });
      await user.click(permissionButton);

      // Verify permission error is displayed
      await waitFor(() => {
        expect(screen.getByText(/permission.*denied/i)).toBeInTheDocument();
      });

      // Verify instructions are shown
      expect(screen.getByText(/system preferences/i)).toBeInTheDocument();
    });

    it("should handle invalid file uploads gracefully", async () => {
      render(<App />);

      // Open import dialog
      const menuButton = screen.getByRole("button", { name: /menu/i });
      await user.click(menuButton);

      const importButton = screen.getByRole("menuitem", { name: /import/i });
      await user.click(importButton);

      // Upload invalid file
      const fileInput = screen.getByLabelText(/import chat file/i);
      const invalidFile = new File(["invalid json content"], "invalid.json", {
        type: "application/json",
      });
      await user.upload(fileInput, invalidFile);

      // Verify error is displayed
      await waitFor(() => {
        expect(screen.getByText(/invalid.*file/i)).toBeInTheDocument();
      });
    });
  });

  describe("Performance Requirements", () => {
    it("should render main interface within performance budget", async () => {
      const renderTime = await performanceUtils.measureRenderTime(() => {
        render(<App />);
      });

      // Should render within 100ms
      expect(renderTime).toBeLessThan(100);
    });

    it("should handle chat scroll performance", async () => {
      render(<App />);

      // Simulate large conversation
      const mockConversation = testUtils.createMockConversation(50);

      // Mock the conversation data
      vi.mocked(window.__TAURI__.core.invoke).mockResolvedValueOnce({
        messages: mockConversation,
      });

      // Trigger conversation load
      const chatContainer = screen.getByTestId("chat-container");

      const scrollTime = await performanceUtils.measureRenderTime(() => {
        fireEvent.scroll(chatContainer, { target: { scrollTop: 1000 } });
      });

      // Scroll should be smooth (< 16ms for 60fps)
      expect(scrollTime).toBeLessThan(16);
    });

    it("should handle rapid user interactions", async () => {
      render(<App />);

      const input = screen.getByRole("textbox", { name: /chat input/i });

      // Rapid typing simulation
      const startTime = performance.now();

      for (let i = 0; i < 100; i++) {
        await user.type(input, "a");
        await performanceUtils.waitForNextFrame();
      }

      const totalTime = performance.now() - startTime;

      // Should handle rapid input without lag (< 1000ms for 100 characters)
      expect(totalTime).toBeLessThan(1000);
    });

    it("should maintain memory usage within limits", async () => {
      const initialMemory = performanceUtils.measureMemoryUsage();

      render(<App />);

      // Simulate extended usage
      for (let i = 0; i < 10; i++) {
        const input = screen.getByRole("textbox", { name: /chat input/i });
        await user.type(input, `Message ${i}`);
        await user.keyboard("{Enter}");
        await testUtils.waitForAsyncOperations();
      }

      const finalMemory = performanceUtils.measureMemoryUsage();
      const memoryIncrease = finalMemory - initialMemory;

      // Memory increase should be reasonable (< 10MB for basic usage)
      expect(memoryIncrease).toBeLessThan(10 * 1024 * 1024);
    });
  });

  describe("Accessibility", () => {
    it("should maintain proper focus order", () => {
      const { container } = render(<App />);

      const focusOrderValid = a11yUtils.checkFocusOrder(container);
      expect(focusOrderValid).toBe(true);
    });

    it("should provide accessible labels for all interactive elements", () => {
      const { container } = render(<App />);

      const a11yIssues = a11yUtils.checkAriaLabels(container);
      expect(a11yIssues).toHaveLength(0);
    });

    it("should support keyboard navigation", async () => {
      render(<App />);

      // Tab through main navigation
      await user.tab();
      expect(
        screen.getByRole("button", { name: /start listening/i })
      ).toHaveFocus();

      await user.tab();
      expect(
        screen.getByRole("textbox", { name: /chat input/i })
      ).toHaveFocus();

      await user.tab();
      expect(screen.getByRole("button", { name: /send/i })).toHaveFocus();

      // Test keyboard shortcuts
      await user.keyboard("{Escape}");
      // Should close any open modals

      testUtils.mockKeyboardShortcut("k", ["cmd"]);
      // Should open command palette or search
    });

    it("should provide proper ARIA live regions for dynamic content", async () => {
      render(<App />);

      // Trigger agent response
      const input = screen.getByRole("textbox", { name: /chat input/i });
      await user.type(input, "Test message");
      await user.keyboard("{Enter}");

      // Check for live region updates
      const liveRegion = screen.getByRole("status");
      expect(liveRegion).toBeInTheDocument();

      await waitFor(() => {
        expect(
          within(liveRegion).getByText(/mock agent response/i)
        ).toBeInTheDocument();
      });
    });

    it("should meet color contrast requirements", () => {
      const { container } = render(<App />);

      const textElements = container.querySelectorAll("p, span, button, input");

      textElements.forEach((element) => {
        const hasGoodContrast = a11yUtils.checkColorContrast(
          element as HTMLElement
        );
        expect(hasGoodContrast).toBe(true);
      });
    });
  });

  describe("Responsive Design", () => {
    it("should adapt to mobile viewport", async () => {
      // Mock mobile viewport
      Object.defineProperty(window, "matchMedia", {
        writable: true,
        value: vi.fn().mockImplementation((query) => ({
          matches: query.includes("max-width: 768px"),
          media: query,
          onchange: null,
          addListener: vi.fn(),
          removeListener: vi.fn(),
          addEventListener: vi.fn(),
          removeEventListener: vi.fn(),
          dispatchEvent: vi.fn(),
        })),
      });

      render(<App />);

      // Mobile-specific elements should be visible
      expect(screen.getByTestId("mobile-menu")).toBeInTheDocument();

      // Desktop-specific elements should be hidden
      expect(screen.queryByTestId("desktop-sidebar")).not.toBeInTheDocument();
    });

    it("should handle window resize events", async () => {
      const { container } = render(<App />);

      // Simulate window resize
      global.innerWidth = 500;
      global.innerHeight = 800;
      fireEvent(window, new Event("resize"));

      await testUtils.waitForAsyncOperations();

      // Layout should adapt to new dimensions
      const chatContainer = container.querySelector(
        '[data-testid="chat-container"]'
      );
      expect(chatContainer).toHaveStyle({
        height: expect.stringContaining("px"),
      });
    });
  });

  describe("State Management", () => {
    it("should persist user preferences", async () => {
      render(<App />);

      // Change theme
      const themeToggle = screen.getByRole("button", { name: /toggle theme/i });
      await user.click(themeToggle);

      // Verify local storage is updated
      expect(localStorage.setItem).toHaveBeenCalledWith("juno-theme", "light");
    });

    it("should restore state on app restart", async () => {
      // Mock stored state
      vi.mocked(localStorage.getItem).mockImplementation((key) => {
        if (key === "juno-theme") return "light";
        if (key === "juno-last-conversation")
          return JSON.stringify([
            {
              role: "user",
              content: "Previous message",
              timestamp: Date.now(),
            },
          ]);
        return null;
      });

      render(<App />);

      // Verify theme is restored
      expect(document.documentElement).toHaveClass("light");

      // Verify conversation is restored
      expect(screen.getByText("Previous message")).toBeInTheDocument();
    });

    it("should handle concurrent state updates", async () => {
      render(<App />);

      // Simulate rapid state changes
      const promises = [
        user.click(screen.getByRole("button", { name: /start listening/i })),
        user.click(screen.getByRole("button", { name: /settings/i })),
        user.click(screen.getByRole("button", { name: /help/i })),
      ];

      await Promise.all(promises);

      // All operations should complete without conflicts
      expect(window.__TAURI__.core.invoke).toHaveBeenCalledTimes(3);
    });
  });
});
