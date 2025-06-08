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
  testClickVisualization: boolean;
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

export type ClickQAResult = {
  success: boolean;
  operation: string;
  coordinates: [number, number];
  original_coordinates?: [number, number];
  error?: string;
  visualization_success: boolean;
  cursor_position_after?: [number, number];
  latency_ms: number;
};

export type CoordinateTestResult = {
  original: { x: number; y: number };
  transformed_to_screen: { x: number; y: number };
  transformed_back: { x: number; y: number };
  error: { x: number; y: number };
  scaling_info?: any;
  is_accurate: boolean;
};

export type VisualizationTestResult = {
  test: string;
  results: Array<{
    position: { x: number; y: number };
    color: string;
    success: boolean;
    error?: string;
  }>;
  success_rate: number;
};
