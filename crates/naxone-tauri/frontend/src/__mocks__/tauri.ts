// Mock Tauri APIs for testing
import { vi } from "vitest";

// Store mock responses keyed by command name
const mockResponses: Record<string, any> = {};

export function setMockResponse(command: string, response: any) {
  mockResponses[command] = response;
}

export function clearMocks() {
  Object.keys(mockResponses).forEach((k) => delete mockResponses[k]);
}

// Mock @tauri-apps/api/core
export const invoke = vi.fn(async (cmd: string, args?: any) => {
  if (cmd in mockResponses) {
    const resp = mockResponses[cmd];
    if (resp instanceof Error) throw resp;
    return typeof resp === "function" ? resp(args) : resp;
  }
  return undefined;
});

// Mock @tauri-apps/api/window
export const getCurrentWindow = () => ({
  minimize: vi.fn(),
  toggleMaximize: vi.fn(),
  isMaximized: vi.fn().mockResolvedValue(false),
  close: vi.fn(),
  show: vi.fn(),
  hide: vi.fn(),
  setFocus: vi.fn(),
});

// Mock @tauri-apps/plugin-dialog
export const open = vi.fn().mockResolvedValue(null);
