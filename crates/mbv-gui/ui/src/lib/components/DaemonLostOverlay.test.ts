// The daemon-lost recovery dialog (`daemon-disconnect-handling`: an
// unannounced connection loss raises a recovery dialog). The dialog paints
// the diagnostics the frame carries and the recovery choices the core
// projects, and a press on a choice reports its region so the core answers
// the choice the window painted. The Tauri IPC is mocked; the asserted
// payloads are the serde wire shapes of bridge/frame/overlay.rs and
// bridge/layout.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import { installInput } from '$lib/bridge/input';
import type { GuiOverlayFrame } from '$lib/bridge/types';
import DaemonLostOverlay from './DaemonLostOverlay.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
});

/** The dialog the core projects: a lost Owner that was playing something,
 * with the three recovery choices the TUI's hint line offers. */
function daemonLost(
	overrides: Partial<Extract<GuiOverlayFrame, { kind: 'daemonLost' }>> = {},
): Extract<GuiOverlayFrame, { kind: 'daemonLost' }> {
	return {
		kind: 'daemonLost',
		lastPlayingTitle: 'Track One',
		daemonLogPath: '/home/user/.local/state/mbv/local-daemon.log',
		restartError: null,
		buttons: [
			{ label: 'Restart and resume', keys: 'R', tone: 'affirmative' },
			{ label: "Restart, don't resume", keys: 'S', tone: 'affirmative' },
			{ label: 'Quit', keys: 'Q', tone: 'cancel' },
		],
		...overrides,
	};
}

/** The region targets of the last merged layout report. */
function lastReportedTargets(): unknown[] {
	const calls = invokeMock.mock.calls.filter((entry) => entry[0] === 'report_layout');
	const report = calls.at(-1)?.[1] as
		| { report: { regions: { target: unknown }[] } }
		| undefined;
	return report?.report.regions.map((entry) => entry.target) ?? [];
}

test('the dialog paints its diagnostics and its recovery choices', async () => {
	const { getByText } = render(DaemonLostOverlay, {
		daemonLost: daemonLost({ restartError: 'attach refused' }),
	});

	expect(getByText('The local daemon connection was lost unexpectedly.')).toBeTruthy();
	expect(getByText('Was playing: Track One')).toBeTruthy();
	expect(getByText('Daemon log: /home/user/.local/state/mbv/local-daemon.log')).toBeTruthy();
	expect(getByText('Restart failed: attach refused')).toBeTruthy();
	expect(getByText('Restart and resume')).toBeTruthy();
	expect(getByText("Restart, don't resume")).toBeTruthy();
	expect(getByText('Quit')).toBeTruthy();

	await vi.waitFor(() =>
		expect(lastReportedTargets()).toEqual([
			{ surface: 'daemonLost', id: { button: 0 } },
			{ surface: 'daemonLost', id: { button: 1 } },
			{ surface: 'daemonLost', id: { button: 2 } },
		]),
	);
});

test('a frame with nothing playing paints no title line', () => {
	const { getByText, queryByText } = render(DaemonLostOverlay, {
		daemonLost: daemonLost({ lastPlayingTitle: null }),
	});

	expect(getByText('The local daemon connection was lost unexpectedly.')).toBeTruthy();
	expect(queryByText(/Was playing:/)).toBeNull();
});

test('the affirmative choice paints the accent text token on the recessed fill', () => {
	const { container } = render(DaemonLostOverlay, { daemonLost: daemonLost() });
	const affirmative = container.querySelector<HTMLElement>('[data-daemon-lost-button="0"]');

	// The dialog paints the confirm modal's pill look: accent text on the
	// dim surface, never the picked row's fill token (the dark-on-green
	// contrast bug this guards).
	expect(affirmative?.style.color).toBe('var(--canvas-accent)');
	expect(affirmative?.style.background).toBe('var(--canvas-well)');
});

test('a press on a choice forwards its press for the core to answer', async () => {
	const { container } = render(DaemonLostOverlay, { daemonLost: daemonLost() });
	const quit = container.querySelector<HTMLElement>('[data-daemon-lost-button="2"]');
	expect(quit?.textContent).toContain('Quit');

	const detach = installInput();
	fireEvent.pointerDown(quit!, { clientX: 12, clientY: 34, button: 0, buttons: 1 });
	detach();

	await vi.waitFor(() =>
		expect(invokeMock).toHaveBeenCalledWith('pointer', {
			event: {
				kind: 'press',
				button: 0,
				buttons: 1,
				x: 12,
				y: 34,
				metaKey: false,
				shiftKey: false,
				ctrlKey: false,
				altKey: false,
				deltaX: 0,
				deltaY: 0,
			},
		}),
	);
	// The press resolves against the region the dialog reported for that
	// choice, so the core answers the button the window painted there.
	expect(lastReportedTargets()).toContainEqual({ surface: 'daemonLost', id: { button: 2 } });
});
