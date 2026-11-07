// add-tauri-gui-client task 6.4: the track chips row paints the video
// stream's label-only chip and the audio and subtitle chips from the
// frame's `trackChips` datum, and the track popover paints the frame's
// `trackPopover` overlay — the subtitle list's Off row first, the chosen
// choice marked — anchored above its chip. The chips report `audioChip`
// and `subtitleChip` regions (the video chip is label-only and reports
// none) and the popover's rows report their `trackPopoverChoice` indices;
// presses forward through the shared input installer as `pointer` events
// and the core resolves them against the hit map. The Tauri IPC is
// mocked, and the asserted payloads are the serde wire shapes of
// bridge/layout.rs and bridge/pointer.rs.

import { invoke } from '@tauri-apps/api/core';
import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, expect, test, vi } from 'vitest';
import { installInput } from '$lib/bridge/input';
import type {
	GuiOverlayFrame,
	GuiTrackChips,
} from '$lib/bridge/types';
import TrackChips from './TrackChips.svelte';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
});

/** The chips the frame carries over a playing video item: a video label,
 * two audio tracks with the first chosen, and one forced subtitle with
 * none chosen (the popover's Off row is then chosen). */
function chips(): GuiTrackChips {
	return {
		video: '1080p',
		audio: {
			choices: [
				{ id: 1, label: 'English', forced: false },
				{ id: 2, label: 'Japanese', forced: false },
			],
			chosen: 1,
		},
		subtitles: {
			choices: [{ id: 1, label: 'English', forced: true }],
			chosen: null,
		},
	};
}

/** The open subtitle popover the frame carries: Off first, then the
 * tracks, the chosen one marked. */
function subtitlePopover(): GuiOverlayFrame {
	return {
		kind: 'trackPopover',
		chip: 'subtitles',
		cursor: 0,
		choices: [
			{ id: 0, label: 'Off', forced: false, chosen: true },
			{ id: 1, label: 'English', forced: true, chosen: false },
		],
	};
}

function renderChips(
	trackChips: GuiTrackChips | null = chips(),
	overlay: GuiOverlayFrame | null = null,
) {
	return render(TrackChips, { chips: trackChips, overlay });
}

/** The `pointer` invoke payloads the test's presses produced. */
function pointerCalls(): { event: { x: number; y: number } }[] {
	return invokeMock.mock.calls
		.filter((call) => call[0] === 'pointer')
		.map((call) => call[1] as { event: { x: number; y: number } });
}

test('a frame renders the video label and the audio and subtitle chips', () => {
	const { getByText } = renderChips();

	expect(getByText('1080p')).toBeTruthy();
	expect(getByText('English')).toBeTruthy();
	expect(getByText('Off')).toBeTruthy();
});

test('an audio-only frame paints no video chip', () => {
	const { queryByText } = renderChips({ ...chips(), video: null });

	expect(queryByText('1080p')).toBeNull();
});

test('no chips paint while the local player is inactive', () => {
	const { container } = renderChips(null);

	expect(container.querySelector('[data-chip]')).toBeNull();
});

interface ReportedReport {
	report: { regions: { target: { id: unknown } }[] };
}

/** The region target ids of the component's last layout report. */
function reportedRegionIds(): unknown[] {
	const calls = invokeMock.mock.calls.filter((entry) => entry[0] === 'report_layout');
	const report = calls.at(-1)?.[1] as ReportedReport | undefined;
	return report?.report.regions.map((entry) => entry.target.id) ?? [];
}

test('the chips report their region ids, the video chip none', async () => {
	renderChips();

	await vi.waitFor(() => expect(invokeMock).toHaveBeenCalled());
	expect(reportedRegionIds()).toEqual(['audioChip', 'subtitleChip']);
});

test('a press on a chip forwards a pointer press with its position', async () => {
	const { container } = renderChips();
	const chip = container.querySelector('[data-chip="audio"]') as HTMLElement;
	const detach = installInput();
	fireEvent.pointerDown(chip, { clientX: 21, clientY: 9, button: 0, buttons: 1 });
	detach();

	await vi.waitFor(() =>
		expect(pointerCalls()).toEqual([
			{
				event: {
					kind: 'press',
					button: 0,
					buttons: 1,
					x: 21,
					y: 9,
					metaKey: false,
					shiftKey: false,
					ctrlKey: false,
					altKey: false,
					deltaX: 0,
					deltaY: 0,
				},
			},
		]),
	);
});

test('the subtitle popover lists Off first and marks the chosen row', () => {
	const { container } = renderChips(chips(), subtitlePopover());
	const rows = container.querySelectorAll('[data-popover-row]');

	expect(rows).toHaveLength(2);
	expect(rows[0].textContent).toContain('Off');
	expect(rows[0].querySelector('svg.lucide-check')).toBeTruthy();
	expect(rows[1].textContent).toContain('English');
	expect(rows[1].querySelector('svg.lucide-check')).toBeNull();
});

test('an open popover reports one trackPopoverChoice region per row, last', async () => {
	renderChips(chips(), subtitlePopover());

	await vi.waitFor(() => expect(invokeMock).toHaveBeenCalled());
	expect(reportedRegionIds()).toEqual([
		'audioChip',
		'subtitleChip',
		{ trackPopoverChoice: 0 },
		{ trackPopoverChoice: 1 },
	]);
});

test('a press on a popover row forwards a pointer press with its position', async () => {
	const { container } = renderChips(chips(), subtitlePopover());
	const row = container.querySelector('[data-popover-row="1"]') as HTMLElement;
	const detach = installInput();
	fireEvent.pointerDown(row, { clientX: 33, clientY: 15, button: 0, buttons: 1 });
	detach();

	await vi.waitFor(() =>
		expect(pointerCalls()).toEqual([
			{
				event: {
					kind: 'press',
					button: 0,
					buttons: 1,
					x: 33,
					y: 15,
					metaKey: false,
					shiftKey: false,
					ctrlKey: false,
					altKey: false,
					deltaX: 0,
					deltaY: 0,
				},
			},
		]),
	);
});
