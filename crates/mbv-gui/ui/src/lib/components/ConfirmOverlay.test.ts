// The shared confirmation modal (`App::ask_confirm`): the message and the
// button row the frame's `confirm` overlay carries. Its buttons paint the
// TUI's pill look — the tone's text colour on the canvas's recessed dark
// fill — so the affirmative choice reads as accent text, never as the
// picked row's fill. The Tauri IPC is mocked; the asserted payloads are
// the serde wire shapes of bridge/frame/overlay.rs and bridge/layout.rs.

import { invoke } from '@tauri-apps/api/core';
import { render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiOverlayFrame } from '$lib/bridge/types';
import ConfirmOverlay from './ConfirmOverlay.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
});

/** The modal the core projects: a question and the two choices the TUI's
 * confirm modal offers. */
function confirm(
	overrides: Partial<Extract<GuiOverlayFrame, { kind: 'confirm' }>> = {},
): Extract<GuiOverlayFrame, { kind: 'confirm' }> {
	return {
		kind: 'confirm',
		message: 'Clear the queue?',
		buttons: [
			{ label: 'Clear', keys: 'Enter', tone: 'affirmative' },
			{ label: 'Cancel', keys: 'Esc', tone: 'cancel' },
		],
		...overrides,
	};
}

test('the modal paints its message and its choices', () => {
	const { getByText } = render(ConfirmOverlay, { confirm: confirm() });

	expect(getByText('Clear the queue?')).toBeTruthy();
	expect(getByText('Clear')).toBeTruthy();
	expect(getByText('Cancel')).toBeTruthy();
});

test('the affirmative choice paints the accent text token on the recessed fill', () => {
	const { container } = render(ConfirmOverlay, { confirm: confirm() });
	const affirmative = container.querySelector<HTMLElement>('[data-confirm-button="0"]');

	// The TUI's affirmative pill is accent text on the dim surface, so the
	// token is the accent text token — never the picked row's fill token
	// (the dark-on-green contrast bug this guards).
	expect(affirmative?.style.color).toBe('var(--canvas-accent)');
	expect(affirmative?.style.background).toBe('var(--canvas-well)');
});
