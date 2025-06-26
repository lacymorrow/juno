import React, { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { FileText, Folder, Info } from "lucide-react";
import { invokeCommand } from "@/lib/utils";

const FileOperations: React.FC = () => {
  const [pathToList, setPathToList] = useState<string>("");
  const [fileListResult, setFileListResult] = useState<string | null>(null);
  const [pathGetContent, setPathGetContent] = useState<string>("");
  const [fileContentResult, setFileContentResult] = useState<string | null>(
    null
  );
  const [pathSetContent, setPathSetContent] = useState<string>("");
  const [fileContentToSet, setFileContentToSet] = useState<string>("");

  const handleListFiles = async () => {
    if (!pathToList.trim()) {
      toast.error("Please enter a path to list.");
      return;
    }
    setFileListResult(null);
    const result = await invokeCommand<string | null>(
      "list_files",
      { pathStr: pathToList.trim() },
      "listFiles"
    );

    if (result !== null && typeof result === "string") {
      try {
        const parsedList = JSON.parse(result);
        const formattedList = parsedList
          .map(
            (entry: { is_dir: boolean; name: string }) =>
              `${entry.is_dir ? "[D]" : "[F]"} ${entry.name}`
          )
          .join("\n");
        setFileListResult(formattedList);
      } catch (parseError) {
        console.error("Failed to parse file list JSON:", parseError);
        setFileListResult(result);
        toast.error("Received file list, but failed to parse or format.");
      }
    } else if (result !== null) {
      setFileListResult(String(result));
      toast.error("Received unexpected non-string result for file list.");
    } else {
      setFileListResult("(Failed to list files)");
    }
  };

  const handleGetFileContent = async () => {
    if (!pathGetContent.trim()) {
      toast.error("Please enter a file path to read.");
      return;
    }
    setFileContentResult(null);
    const result = await invokeCommand<string | null>(
      "get_file_content",
      { pathStr: pathGetContent.trim() },
      "getFileContent"
    );
    if (result !== null) {
      setFileContentResult(result);
    } else {
      setFileContentResult("(Failed to get file content)");
    }
  };

  const handleSetFileContent = async () => {
    if (!pathSetContent.trim()) {
      toast.error("Please enter a file path to write to.");
      return;
    }
    await invokeCommand(
      "set_file_content",
      { pathStr: pathSetContent.trim(), content: fileContentToSet },
      "setFileContent"
    );
  };

  return (
    <div className="space-y-4">
      <div className="bg-blue-50 border border-blue-200 rounded-lg p-3 mb-4">
        <div className="flex items-start space-x-2">
          <Info className="h-4 w-4 text-blue-600 mt-0.5 flex-shrink-0" />
          <div className="text-sm text-blue-800">
            <strong>Tool Consolidation:</strong> This component now uses
            production file operation functions with built-in debug capabilities
            instead of dev_* functions.
          </div>
        </div>
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <Folder className="h-4 w-4" />
          <Input
            placeholder="Path to list files"
            value={pathToList}
            onChange={(e) => setPathToList(e.target.value)}
          />
          <Button onClick={handleListFiles}>List Files</Button>
        </div>
        {fileListResult && (
          <pre className="mt-2 whitespace-pre-wrap break-all text-sm">
            {fileListResult}
          </pre>
        )}
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <FileText className="h-4 w-4" />
          <Input
            placeholder="Path to read file"
            value={pathGetContent}
            onChange={(e) => setPathGetContent(e.target.value)}
          />
          <Button onClick={handleGetFileContent}>Get Content</Button>
        </div>
        {fileContentResult && (
          <pre className="mt-2 whitespace-pre-wrap break-all text-sm">
            {fileContentResult}
          </pre>
        )}
      </div>

      <div className="space-y-2">
        <div className="flex items-center space-x-2">
          <FileText className="h-4 w-4" />
          <Input
            placeholder="Path to write file"
            value={pathSetContent}
            onChange={(e) => setPathSetContent(e.target.value)}
          />
          <Button onClick={handleSetFileContent}>Set Content</Button>
        </div>
        <Textarea
          placeholder="Content to write"
          value={fileContentToSet}
          onChange={(e) => setFileContentToSet(e.target.value)}
          rows={4}
        />
      </div>
    </div>
  );
};

export default FileOperations;
