export type LoadingStates = {
  screenshot: boolean;
  focusInfo: boolean;
  focusDelay: boolean;
  elementScreenshot: boolean;
  clickFocus: boolean;
  typeText: boolean;
  pressKey: boolean;
  openApp: boolean;
  openUrl: boolean;
  scroll: boolean;
  globalTypeText: boolean;
  getClipboard: boolean;
  setClipboard: boolean;
  holdKey: boolean;
  releaseKey: boolean;
  wait: boolean;
  findElement: boolean;
  clickElement: boolean;
  getSelectedText: boolean;
  getWindowList: boolean;
  getWindowInfo: boolean;
  focusWindow: boolean;
  resizeWindow: boolean;
  moveWindow: boolean;
  closeWindow: boolean;
  listFiles: boolean;
  getFileContent: boolean;
  setFileContent: boolean;
  mouseMove: boolean;
  mouseDown: boolean;
  mouseUp: boolean;
  mouseClick: boolean;
  mouseDoubleClick: boolean;
  mouseDrag: boolean;
  setDeveloperPlayback: boolean;
  playbackAudio: boolean;
  setTtsProvider: boolean;
  testSystemContext: boolean;
  // Always Listening Testing
  debugAlwaysListening: boolean;
  startAlwaysListening: boolean;
  stopAlwaysListening: boolean;
  toggleAlwaysListening: boolean;
  setAlwaysListeningSensitivity: boolean;
  setAlwaysListeningWakeWords: boolean;
};

export type FileEntry = {
  name: string;
  is_dir: boolean;
};

export type ToolUsageEntry = {
  timestamp: number;
  tool: string;
  inputs: Record<string, any>;
  result?: any;
  success: boolean;
  screenshot_base64?: string;
  show_timestamp: boolean;
  formatted_time?: string;
};


