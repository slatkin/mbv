// add-gui-library-panel row 4.3: the top bar paints one tab per presented
// destination, marks the current one, and a press sends the open-destination
// command with the tab's position (design D2). Phase 1 presents Music only, so
// no Home icon and no Search field (task 4.3's later groups). The Tauri IPC is
// mocked; the asserted payload is the serde wire shape of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import LibraryTopBar from './LibraryTopBar.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
});

test('paints one tab per presented destination and marks the current one', () => {
	const { container } = render(LibraryTopBar, {
		tabs: [
			{ position: 1, label: 'Music' },
			{ position: 2, label: 'Shows' },
		],
		currentTab: 2,
	});
	const tabs = [...container.querySelectorAll('[data-library-tab]')];

	expect(tabs.map((tab) => tab.textContent?.trim())).toEqual(['Music', 'Shows']);
	expect(tabs.map((tab) => tab.getAttribute('aria-current'))).toEqual([null, 'page']);
});

test('a press opens the destination tab at its position', () => {
	const { container } = render(LibraryTopBar, {
		tabs: [
			{ position: 1, label: 'Music' },
			{ position: 3, label: 'Audiobooks' },
		],
		currentTab: 1,
	});

	fireEvent.click(container.querySelectorAll('[data-library-tab]')[1] as HTMLElement);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'openDestination', position: 3 },
	});
});
