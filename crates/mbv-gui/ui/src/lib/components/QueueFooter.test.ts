// add-tauri-gui-client task 6.4: the canvas footer line paints the queue
// source pills, the session marker (the footer owner pill's label, with no
// liveness dot — the frame carries no liveness field), the prefix-armed
// state, and the toast inline in the line in its severity colour. It
// reports the playlist pill's region through `report_layout` (design D4).
// The Tauri IPC is mocked; the asserted payload is the serde wire shape of
// bridge/layout.rs.

import { invoke } from '@tauri-apps/api/core';
import { render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiQueueFooter, GuiStatusBar } from '$lib/bridge/types';
import QueueFooter from './QueueFooter.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

/** A ResizeObserver stub: jsdom has none and no layout. */
class ResizeObserverStub {
	observe() {}
	unobserve() {}
	disconnect() {}
}

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	vi.stubGlobal('ResizeObserver', ResizeObserverStub);
});

const footer: GuiQueueFooter = {
	owner: { text: 'woims', role: 'textFocusAccent', bold: false },
	playlist: { text: 'mix', role: 'textMetadata', bold: false },
	autosave: null,
	dirty: null,
};

const idle: GuiStatusBar = { prefixArmed: false, toast: null };

function renderFooter(
	foot: GuiQueueFooter = footer,
	status: GuiStatusBar = idle,
) {
	return render(QueueFooter, { footer: foot, status });
}

test('the footer renders the queue source and the session marker', () => {
	const { getByText, container } = renderFooter();

	expect(getByText('mix')).toBeTruthy();
	expect(getByText('woims')).toBeTruthy();
	// The session marker is the owner pill's label only: the frame carries
	// no liveness field, so no liveness dot may paint.
	expect(container.textContent).not.toContain('●');
});

// The footer pill's icon contract (the layout-parity brief): the GUI maps
// each pill's slot to its Lucide icon — the playlist source's
// `playlist-play` glyph maps to `list-music`, the owner indicator's
// `connection` to `cable`, the autosave `content-save-check` to
// `save-check`, and the dirty `content-save-alert` to `save-off` — where
// the TUI paints its Nerd Font glyphs. Lucide renders each icon's name as
// the svg's `lucide-<name>` class.
test('the footer pills paint their Lucide icons', () => {
	const { container } = renderFooter({
		owner: { text: 'woims', role: 'textFocusAccent', bold: false },
		playlist: { text: 'mix', role: 'textMetadata', bold: false },
		autosave: { text: 'AUTOSAVE', role: 'accent', bold: false },
		dirty: { text: 'UNSAVED', role: 'textFocusAccent', bold: true },
	});

	for (const icon of [
		'lucide-list-music',
		'lucide-cable',
		'lucide-save-check',
		'lucide-save-off',
	]) {
		expect(container.querySelector(`svg.${icon}`), `${icon} paints`).toBeTruthy();
	}
});

test('an armed frame renders the prefix badge', () => {
	const { getByText } = renderFooter(footer, { prefixArmed: true, toast: null });

	expect(getByText('PREFIX')).toBeTruthy();
});

test('a live toast paints inline in the line and drops the prefix badge', () => {
	const { getByText, queryByText } = renderFooter(footer, {
		prefixArmed: true,
		toast: { text: "Scanning 'Movies'...", severity: 'neutral' },
	});

	expect(getByText("Scanning 'Movies'...")).toBeTruthy();
	expect(queryByText('PREFIX'), 'the prefix badge drops while a toast is live').toBeNull();
});

test('the line reports the playlist pill region', async () => {
	renderFooter();

	await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));
	expect(invokeMock).toHaveBeenCalledWith('report_layout', {
		report: {
			lists: [],
			regions: [
				{
					rect: { x: 0, y: 0, width: 0, height: 0 },
					target: { surface: 'queue', id: 'playlistPill' },
				},
			],
			containerSize: { width: 0, height: 0 },
		},
	});
});
