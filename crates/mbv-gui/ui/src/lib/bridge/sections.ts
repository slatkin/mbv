// The window's section stores (gui-change-driven-sections design D6): one
// writable store for each window section, fed by the one `mbv://sections`
// event. `applyBatch` writes a section's store only when the batch's revision
// differs from the one the store holds, so a `sync` re-send of a section the
// window already holds writes nothing. A batch that carries `theme` applies
// the role colours, as the frame event did (design D7).
//
// The stores hold no interaction state — the core thread owns that — so a
// window reload re-subscribes, calls `sync`, and repaints from the full batch
// the core answers with (design D4). Components read their sections through
// Svelte's store contract (`$playback`); a store write notifies only that
// store's subscribers, so a `queue` batch re-renders only the queue.

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { writable } from 'svelte/store';
import type { Writable } from 'svelte/store';
import { playhead } from './playhead';
import { applyTheme } from './theme';
import type {
	GuiAttachState,
	GuiFocus,
	GuiLibraryContent,
	GuiOverlayFrame,
	GuiPanelToggles,
	GuiQueue,
	GuiQueueFooter,
	GuiQueuePlayback,
	GuiQueueSummary,
	GuiScrubberHover,
	GuiSearchFrame,
	GuiSectionBatch,
	GuiSettingsFrame,
	GuiStatusBar,
	GuiTheme,
	SectionName,
	SectionValues,
} from './types';

/** The event the core thread emits its section batches through; the
 * `mbv://sections` sink in main.rs. */
export const SECTIONS_EVENT = 'mbv://sections';

/** The value each section store holds before the first batch arrives. The
 * window paints no main content before that batch, so these are the bare
 * values a test starts from, not painted state. */
const FRESH: Omit<SectionValues, 'theme'> & { theme: GuiTheme | null } = {
	attach: null,
	theme: null,
	focus: { surface: null, panel: 'queue' },
	queue: { rows: [], cursor: null, marks: [], active: null, hover: null, drag: null },
	summary: {
		remaining: 0,
		nextTitle: null,
		tailSecs: null,
		rightColumn: 'length',
		filter: '',
		filterActive: false,
	},
	panel: { collapsed: false, queueFolded: false, uiResetEpoch: 0 },
	overlay: null,
	playback: {
		status: 'idle',
		host: '',
		hostIsRemote: false,
		shown: false,
		showControls: false,
		nowPlayingTitle: null,
		parentLine: null,
		episodeBadge: null,
		stillCaption: null,
		chapters: [],
		introMark: null,
		creditsMark: null,
		trickplay: null,
		availability: { stop: false, next: false, previous: false },
		statusIndicators: null,
		artwork: null,
		trackChips: null,
		targetChoices: [],
	},
	hover: null,
	footer: {
		owner: { text: '', role: 'textPrimary', bold: false },
		ownerLive: false,
		playlist: { text: '', role: 'textPrimary', bold: false },
		autosave: null,
		dirty: null,
	},
	status: { prefixArmed: false, toast: null },
	library: null,
	search: null,
	settings: null,
};

/** The pre-attach state the window shows, `null` once the attach succeeds and
 * before the first batch lands. */
export const attach = writable<GuiAttachState | null>(FRESH.attach);

/** The role-to-colour map of the last carried theme section, `null` before the
 * first batch. The colours the window paints live in the CSS custom
 * properties `applyTheme` writes. */
export const theme = writable<GuiTheme | null>(null);

export const focus = writable<GuiFocus>(FRESH.focus);

export const queue = writable<GuiQueue>(FRESH.queue);

export const summary = writable<GuiQueueSummary>(FRESH.summary);

export const panel = writable<GuiPanelToggles>(FRESH.panel);

export const overlay = writable<GuiOverlayFrame | null>(FRESH.overlay);

export const playback = writable<GuiQueuePlayback>(FRESH.playback);

/** The scrubber's hover preview, `null` while the pointer is away from the
 * bar. */
export const hover = writable<GuiScrubberHover | null>(FRESH.hover);

export const footer = writable<GuiQueueFooter>(FRESH.footer);

export const status = writable<GuiStatusBar>(FRESH.status);

/** The Library content of the active destination, `null` while no presented
 * destination is active. */
export const library = writable<GuiLibraryContent | null>(FRESH.library);

/** The Search section's results, `null` while the field is closed. */
export const search = writable<GuiSearchFrame | null>(FRESH.search);

/** The Settings screen, `null` while the screen is closed. */
export const settings = writable<GuiSettingsFrame | null>(FRESH.settings);

/** Whether a section batch has arrived: the window paints the main content
 * while this holds and the `attach` store is `null`, and the attach screen
 * before that — the wait before the first batch is not the attached state's
 * own `null` (design D1). */
export const connected = writable(false);

/** The one store per section, keyed by the section name: the table
 * `applyBatch` dispatches through. Every window section has its arm. */
const stores: Record<SectionName, Writable<unknown>> = {
	attach,
	theme,
	focus,
	queue,
	summary,
	panel,
	overlay,
	playback,
	hover,
	footer,
	status,
	library,
	search,
	settings,
};

/** The section names in the store table's order. */
const SECTION_NAMES = Object.keys(stores) as SectionName[];

/** The revision of each section the window last wrote: a batch section at a
 * revision the window holds writes nothing (design D1). */
const held: Partial<Record<SectionName, number>> = {};

/** The revision of the `playhead` section the window last stamped (design
 * D5): the anchor rides the batch but not the section stores, so its
 * revision is tracked beside `held`. A re-sent section whose revision the
 * window holds — a `sync` snapshot's copy — re-stamps nothing, and the
 * prediction continues from the stamp it already holds. */
let heldPlayhead: number | undefined;

/** Write a batch into the section stores (design D6): each present section
 * whose revision the window does not hold replaces its store's value, and a
 * `theme` section also writes the role colours into the CSS custom
 * properties. A carried `playhead` section stamps the prediction stores
 * (design D5) at its own revision. Sections the batch leaves out leave
 * their stores as they are. */
export function applyBatch(batch: GuiSectionBatch): void {
	connected.set(true);
	const carried = batch.playhead;
	if (carried !== undefined && carried.revision !== heldPlayhead) {
		heldPlayhead = carried.revision;
		playhead.stamp(carried.value);
	}
	for (const name of SECTION_NAMES) {
		const section = batch[name];
		if (section === undefined || section.revision === held[name]) {
			continue;
		}
		held[name] = section.revision;
		stores[name].set(section.value);
	}
	if (batch.theme !== undefined) {
		applyTheme(batch.theme.value);
	}
}

/** Subscribe to `mbv://sections` and ask the core for the full snapshot
 * (design D4): the `sync` invoke runs only after the `listen` promise
 * resolves, so no batch can fall between the subscribe and the snapshot. The
 * caller runs the unlisten function on unmount. */
export async function connectSections(): Promise<UnlistenFn> {
	const unlisten = await listen<GuiSectionBatch>(SECTIONS_EVENT, (event) =>
		applyBatch(event.payload),
	);
	await invoke('sync');
	return unlisten;
}

/** Drop every section store and revision back to the fresh-window state, so
 * a test starts from the values the first batch would carry. The playhead
 * drops with them: its anchor and clocks stop, and the next `applyBatch`
 * stamps it again. */
export function resetSectionsForTest(): void {
	playhead.reset();
	heldPlayhead = undefined;
	for (const name of SECTION_NAMES) {
		held[name] = undefined;
	}
	for (const name of SECTION_NAMES) {
		const value: unknown = FRESH[name];
		// The table's key and the fresh value name the same section, so the
		// types agree; a clone keeps the written object distinct from `FRESH`.
		stores[name].set(
			typeof value === 'object' && value !== null
				? structuredClone(value)
				: value,
		);
	}
	connected.set(false);
}
