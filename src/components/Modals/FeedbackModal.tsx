import React from "react";
import type { FeedbackData } from "@/types/chat";

interface FeedbackModalProps {
  onClose: () => void;
  feedbackData: FeedbackData;
  setFeedbackData: React.Dispatch<React.SetStateAction<FeedbackData>>;
  onSubmit: () => void;
}

export const FeedbackModal: React.FC<FeedbackModalProps> = ({
  onClose,
  feedbackData,
  setFeedbackData,
  onSubmit,
}) => {
  return (
    <div className="bg-white dark:bg-gray-800 rounded-lg p-6 max-w-md">
      <div className="flex justify-between items-center mb-4">
        <h2 className="text-xl font-bold text-gray-900 dark:text-white">
          Submit Feedback
        </h2>
        <button
          onClick={onClose}
          className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
        >
          <svg
            className="w-6 h-6"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
        </button>
      </div>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          onSubmit();
        }}
        className="space-y-4"
      >
        <div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            Feedback Type
          </label>
          <select
            value={feedbackData.type}
            onChange={(e) =>
              setFeedbackData((prev) => ({
                ...prev,
                type: e.target.value as "issue" | "feature" | "general",
              }))
            }
            className="w-full p-2 border border-gray-300 rounded-md dark:border-gray-600 dark:bg-gray-700 dark:text-white"
          >
            <option value="general">General Feedback</option>
            <option value="issue">Bug Report</option>
            <option value="feature">Feature Request</option>
          </select>
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            Title *
          </label>
          <input
            type="text"
            value={feedbackData.title}
            onChange={(e) =>
              setFeedbackData((prev) => ({
                ...prev,
                title: e.target.value,
              }))
            }
            className="w-full p-2 border border-gray-300 rounded-md dark:border-gray-600 dark:bg-gray-700 dark:text-white"
            placeholder="Brief summary of your feedback"
            required
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            Description *
          </label>
          <textarea
            value={feedbackData.description}
            onChange={(e) =>
              setFeedbackData((prev) => ({
                ...prev,
                description: e.target.value,
              }))
            }
            className="w-full p-2 border border-gray-300 rounded-md dark:border-gray-600 dark:bg-gray-700 dark:text-white h-24"
            placeholder="Detailed description of your feedback"
            required
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            Priority
          </label>
          <select
            value={feedbackData.priority}
            onChange={(e) =>
              setFeedbackData((prev) => ({
                ...prev,
                priority: e.target.value as "low" | "medium" | "high",
              }))
            }
            className="w-full p-2 border border-gray-300 rounded-md dark:border-gray-600 dark:bg-gray-700 dark:text-white"
          >
            <option value="low">Low</option>
            <option value="medium">Medium</option>
            <option value="high">High</option>
          </select>
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            Email (Optional)
          </label>
          <input
            type="email"
            value={feedbackData.email}
            onChange={(e) =>
              setFeedbackData((prev) => ({
                ...prev,
                email: e.target.value,
              }))
            }
            className="w-full p-2 border border-gray-300 rounded-md dark:border-gray-600 dark:bg-gray-700 dark:text-white"
            placeholder="your.email@example.com"
          />
        </div>
        <div className="flex gap-3 pt-2">
          <button
            type="button"
            onClick={onClose}
            className="flex-1 px-4 py-2 border border-gray-300 rounded-md text-gray-700 dark:text-gray-300 dark:border-gray-600 hover:bg-gray-50 dark:hover:bg-gray-700"
          >
            Cancel
          </button>
          <button
            type="submit"
            className="flex-1 px-4 py-2 bg-blue-500 text-white rounded-md hover:bg-blue-600"
          >
            Submit
          </button>
        </div>
      </form>
    </div>
  );
};