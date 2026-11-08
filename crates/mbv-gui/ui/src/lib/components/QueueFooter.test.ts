// add-tauri-gui-client task 6.4: the canvas footer line paints the queue
// source pills, the session marker (the footer owner pill's label and, while
// the shown Owner's link is up, its liveness dot), the prefix-armed state,
// and the toast inline in the line in its severity colour. It reports the
// playlist pill's region through `report_layout` (design D4). The Tauri IPC
// is mocked; the asserted payload is the serde wire shape of
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
	ownerLive: true,
	playlist: { text: 'mix', role: 'textMetadata', bold: false },
	autosave: null,
	dirty: null,
};

const idle: GuiStatusBar = { prefixArmed: false, toast: null };

function renderFooter(
	foot: GuiQueueFooter = footer,
	status: GuiStatusBar = idle,
) {
	return render(QueueFooter, { footer: foot, status, libraryVisible: false });
}

test('the footer renders the queue source and the session marker', () => {
	const { getByText, container } = renderFooter();

	expect(getByText('mix')).toBeTruthy();
	expect(getByText('woims')).toBeTruthy();
	// The session marker's dot marks the live Owner (design D4a) and follows
	// the label, as the canvas paints the marker.
	const ownerGroup = container.querySelector('[data-owner-label]')?.parentElement as HTMLElement;
	expect(ownerGroup.lastElementChild?.textContent).toBe('●');
});

// The session marker's dot is the live-Owner mark: an owner whose link is
// down paints none (design D4a: "the dot marks the live Owner").
test('an owner whose link is down paints no liveness dot', () => {
	const { container } = renderFooter({ ...footer, ownerLive: false });

	expect(container.querySelector('[data-owner-live]')).toBeNull();
});

// The footer's canvas form (U4.3): a 40 px band, 16 px side padding, the
// canvas's mono type, a 1 px top border, and the canvas's two line colours
// — the source text in its teal and the owner marker in its sand — from the
// queue's `--canvas-*` tokens.
test('the line paints the canvas footer band', () => {
	const { container } = renderFooter();
	const line = container.querySelector('[data-footer-line]') as HTMLElement;

	expect(line.style.height).toBe('40px');
	expect(line.style.paddingInline).toBe('16px');
	expect(line.style.fontSize).toBe('12px');
	expect(line.style.borderTop).toBe('1px solid var(--canvas-border)');

	const source = container.querySelector('[data-footer-source]') as HTMLElement;
	const ownerGroup = container.querySelector('[data-owner-label]')?.parentElement as HTMLElement;
	expect(source.style.color).toBe('var(--canvas-source)');
	expect(ownerGroup.style.color).toBe('var(--canvas-owner)');
});

// The canvas footer paints the queue source and the owner marker as plain
// text: no leading icon on either.
test('the source text and the owner marker paint with no leading icon', () => {
	const { container } = renderFooter();
	const source = container.querySelector('[data-footer-source]') as HTMLElement;
	const ownerGroup = container.querySelector('[data-owner-label]')?.parentElement as HTMLElement;

	expect(source.parentElement?.querySelector('svg')).toBeNull();
	expect(ownerGroup.querySelector('svg')).toBeNull();
});

// The remaining footer pills map each active state's slot to its Lucide icon
// — the autosave `content-save-check` glyph to `save-check` and the dirty
// `content-save-alert` to `save-off` (the canvas paints neither, so only
// these two icons remain). Lucide renders each icon's name as the svg's
// `lucide-<name>` class.
test('the active footer states paint their Lucide icons', () => {
	const { container } = renderFooter({
		owner: { text: 'woims', role: 'textFocusAccent', bold: false },
		ownerLive: true,
		playlist: { text: 'mix', role: 'textMetadata', bold: false },
		autosave: { text: 'AUTOSAVE', role: 'accent', bold: false },
		dirty: { text: 'UNSAVED', role: 'textFocusAccent', bold: true },
	});

	for (const icon of ['lucide-save-check', 'lucide-save-off']) {
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

test('the line reports no region: its pills are informational', async () => {
	renderFooter();

	await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));
	// A reported target whose press can never fire would lie in the layout
	// report, so the line reports its contribution empty.
	expect(invokeMock).toHaveBeenCalledWith('report_layout', {
		report: {
			lists: [],
			regions: [],
			containerSize: { width: 0, height: 0 },
		},
	});
});
