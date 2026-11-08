// add-gui-library-panel row 4.4: the pill row's multi-select rules (design
// D6). One table of gestures and the exact selected set each one sends, in
// pill order: a press toggles, an Alt-press solos, turning off the last
// selected pill leaves the empty show-all set, clear keeps the first selected
// pill, and a single-select pill replaces the set. The Tauri IPC is mocked;
// the asserted payload is the serde wire shape of bridge/library.rs.

import { invoke } from '@tauri-apps/api/core';
import { beforeEach, expect, test, vi } from 'vitest';
import type { GuiMusicPill } from '$lib/bridge/types';
import type { PillGesture } from './pills';
import { selectLibraryPills } from './state';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

/** The Grouped Music library index the set-collections command names. */
const LIB_IDX = 2;

const HIP_HOP: GuiMusicPill = {
	groupId: 'g1',
	name: 'Hip-Hop',
	selected: true,
	collection: true,
};
const JAZZ: GuiMusicPill = {
	groupId: 'g2',
	name: 'Jazz',
	selected: false,
	collection: true,
};
const SOUL: GuiMusicPill = {
	groupId: 'g3',
	name: 'Soul',
	selected: false,
	collection: true,
};
const LATEST: GuiMusicPill = {
	groupId: 'latest',
	name: 'Latest',
	selected: false,
	collection: false,
};

/** The pills with Hip-Hop alone selected. */
const ONE_SELECTED: GuiMusicPill[] = [HIP_HOP, JAZZ, SOUL];

/** The pills with Hip-Hop and Jazz selected. */
const TWO_SELECTED: GuiMusicPill[] = [
	HIP_HOP,
	{ ...JAZZ, selected: true },
	SOUL,
];

/** The pills of a destination whose Latest pill is single-select. */
const WITH_LATEST: GuiMusicPill[] = [
	HIP_HOP,
	{ ...JAZZ, selected: true },
	LATEST,
];

interface Row {
	readonly case: string;
	readonly gesture: PillGesture;
	readonly groupIds: string[];
	readonly pills?: GuiMusicPill[];
}

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
});

test.each<Row>([
	{
		case: 'a press selects an unselected pill after the selected ones',
		gesture: { kind: 'press', groupId: 'g2', solo: false },
		groupIds: ['g1', 'g2'],
	},
	{
		case: 'a press deselects a selected pill',
		gesture: { kind: 'press', groupId: 'g1', solo: false },
		groupIds: ['g2'],
		pills: TWO_SELECTED,
	},
	{
		case: 'turning off the last selected pill leaves the empty show-all set',
		gesture: { kind: 'press', groupId: 'g1', solo: false },
		groupIds: [],
	},
	{
		case: 'an Alt-press solos the pressed pill',
		gesture: { kind: 'press', groupId: 'g3', solo: true },
		groupIds: ['g3'],
		pills: TWO_SELECTED,
	},
	{
		case: 'clear keeps the first selected pill in pill order',
		gesture: { kind: 'clear' },
		groupIds: ['g1'],
		pills: TWO_SELECTED,
	},
	{
		case: 'clear with one selected pill keeps it',
		gesture: { kind: 'clear' },
		groupIds: ['g1'],
	},
	{
		case: 'a single-select pill clears the set and selects alone',
		gesture: { kind: 'press', groupId: 'latest', solo: false },
		groupIds: ['latest'],
		pills: WITH_LATEST,
	},
])('$case', ({ gesture, groupIds, pills }) => {
	selectLibraryPills(LIB_IDX, pills ?? ONE_SELECTED, gesture);

	expect(invokeMock).toHaveBeenCalledWith('library', {
		command: { command: 'setCollections', libIdx: LIB_IDX, groupIds },
	});
});
