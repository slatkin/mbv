// add-tauri-gui-client task 3.4: the Status bar paints the prefix-armed
// state and lets a live toast take over the row (design D4). It reports
// through the shared layout reporter; the slice's `GuiStatusBar` carries no
// interactive pill content, so its region set is empty. The Tauri IPC is
// mocked; the asserted payload is the serde wire shape of bridge/layout.rs.

import { invoke } from '@tauri-apps/api/core';
import { render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import StatusBar from './StatusBar.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
});

test('an armed frame renders the prefix pill', () => {
	const { getByText } = render(StatusBar, {
		status: { prefixArmed: true, toast: null },
	});

	expect(getByText('PREFIX')).toBeTruthy();
});

test('a live toast takes over the row', () => {
	const { getByText, queryByText } = render(StatusBar, {
		status: {
			prefixArmed: true,
			toast: { text: "Scanning 'Movies'...", severity: 'neutral' },
		},
	});

	expect(getByText("Scanning 'Movies'...")).toBeTruthy();
	expect(
		queryByText('PREFIX'),
		'the prefix pill drops while a toast is live',
	).toBeNull();
});

test('the bar reports its empty region set', async () => {
	render(StatusBar, {
		status: { prefixArmed: false, toast: null },
	});
	await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

	expect(invokeMock).toHaveBeenCalledWith('report_layout', {
		report: { lists: [], regions: [], width: 1024 },
	});
});
