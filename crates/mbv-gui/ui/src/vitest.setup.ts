// The Tauri APIs resolve `window.__TAURI_INTERNALS__`, which exists only
// inside the webview. Every test runs against these mocks instead; a test
// asserts on them through `vi.mocked(invoke)` / `vi.mocked(listen)`.

import { vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn().mockResolvedValue(() => {}),
	once: vi.fn().mockResolvedValue(() => {}),
	emit: vi.fn().mockResolvedValue(undefined),
}));
