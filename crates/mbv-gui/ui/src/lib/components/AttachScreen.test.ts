// add-tauri-gui-client task 3.5: the attach screen paints each
// `GuiAttachState` (design D9) — the wait before the first frame and while
// an Owner shuts down, the two refusal texts, the build mismatch with its
// Restart and Quit buttons, and the attach-failed text — and reports the
// buttons' regions through `report_layout`. The Tauri IPC is mocked; the
// asserted payload is the serde wire shape of bridge/layout.rs.

import { invoke } from '@tauri-apps/api/core';
import { render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiAttachState } from '$lib/bridge/types';
import AttachScreen from './AttachScreen.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

/** A ResizeObserver stub: jsdom has none and no layout. */
class ResizeObserverStub {
	static instances: ResizeObserverStub[] = [];
	constructor() {
		ResizeObserverStub.instances.push(this);
	}
	observe() {}
	disconnect() {}
}

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
	ResizeObserverStub.instances = [];
	vi.stubGlobal('ResizeObserver', ResizeObserverStub);
});

const WAITING_TEXT = 'Attaching to the Owner process…';

test('no frame yet renders the waiting state', () => {
	const { getByText } = render(AttachScreen, { attach: null });

	expect(getByText(WAITING_TEXT)).toBeTruthy();
});

test('a shutting-down Owner renders the waiting state', () => {
	const { getByText } = render(AttachScreen, {
		attach: { kind: 'waiting' } satisfies GuiAttachState,
	});

	expect(getByText(WAITING_TEXT)).toBeTruthy();
});

test('a refusing Owner renders its refusal text', () => {
	const { getByText } = render(AttachScreen, {
		attach: { kind: 'refusal', detail: 'the refusal text' },
	});

	expect(getByText('the refusal text')).toBeTruthy();
});

test('an exclusive Owner renders its refusal text', () => {
	const { getByText } = render(AttachScreen, {
		attach: { kind: 'exclusiveRefusal', detail: 'the exclusive text' },
	});

	expect(getByText('the exclusive text')).toBeTruthy();
});

test('a build mismatch renders its text and the Restart and Quit buttons', () => {
	const { getByText } = render(AttachScreen, {
		attach: {
			kind: 'mismatch',
			detail: 'the mismatch text\nRestarting stops playback and closes any other mbv terminals.',
		},
	});

	expect(getByText(/the mismatch text/)).toBeTruthy();
	expect(getByText('Restart')).toBeTruthy();
	expect(getByText('Quit')).toBeTruthy();
});

test('an attach failure renders its text', () => {
	const { getByText } = render(AttachScreen, {
		attach: { kind: 'failed', detail: 'failed to attach to Owner process: nope' },
	});

	expect(getByText('failed to attach to Owner process: nope')).toBeTruthy();
});

test('the mismatch buttons appear in the reported regions', async () => {
	render(AttachScreen, {
		attach: { kind: 'mismatch', detail: 'the mismatch text' },
	});

	await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

	expect(invokeMock).toHaveBeenCalledWith('report_layout', {
		report: {
			lists: [],
			regions: [
				{
					rect: { x: 0, y: 0, width: 0, height: 0 },
					target: { surface: 'attach', id: 'restart' },
				},
				{
					rect: { x: 0, y: 0, width: 0, height: 0 },
					target: { surface: 'attach', id: 'quit' },
				},
			],
			containerSize: { width: 0, height: 0 },
		},
	});
});
