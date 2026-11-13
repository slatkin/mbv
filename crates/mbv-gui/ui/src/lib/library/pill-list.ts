// The pill-list controller the Feeds, Podcasts, and Emby destinations share
// (add-gui-library-panel row 7C.1, the /simplify review of PR 993): the
// interaction state the window content owns and the core never reads — the
// cursor, the pill row's focus, and the `/` filter's text — plus the actions
// over it. The scroll offset and the measured viewport live in the list's
// scroll store (`./scroll`, design D9), which the controller is given. The core owns the pill
// selection and the selected row (the frame's `pills` and `selected`), so
// nothing here mirrors them: a press sends the select command, and the frame
// carries the choice back. The panel's pane focus and the Hero overlay stay in
// `./state`'s shared store.
//
// Each destination's module keeps its own doc, its own command shapes, its own
// names, and its own extras — Emby's letter rail and play-next — over this one
// controller, so a filter change, a cursor move, a measure, a scroll, and a
// pill walk resolve the same way on every one of them and the three cannot
// drift.

import { get, writable } from 'svelte/store';
import type { Writable } from 'svelte/store';
import { sendLibraryCommand } from '$lib/bridge/library';
import type { GuiLibraryCommand } from '$lib/bridge/types';
import { clampScrollTop, listWindow, scrollTopToShowCursor, stepPillFocus } from './list';
import type { GeometryRow, ListMove } from './list';
import { libraryUi } from './state';
import type { ListScroll } from './scroll';

/** A pill-list destination's interaction state (design D12): what the window
 * content owns and the core never reads. */
export interface PillListUiState<Pill, Focus = Pill> {
	/** The cursor's row id, or `null` while no row carries the cursor
	 * (design D12). The cursor moves over the item rows only. */
	cursor: string | null;
	/** The pill the pill row's Left and Right move the focus to, or `null`
	 * while no pill has it (design D6, D12): the pill itself, or its stable
	 * key — the destination picks. */
	focusedPill: Focus | null;
	/** The pill selection the keys asked for: Space and Enter select the
	 * focused pill, and the pill row turns it into the set-pill command. */
	pillSelect: Pill | null;
	/** The `/` filter's text: the list paints only the rows whose title
	 * contains it, without case (design D7's filter rule). */
	filter: string;
}

/** The state a freshly mounted pill list starts with. */
export function initialPillListUi<Pill, Focus = Pill>(): PillListUiState<Pill, Focus> {
	return {
		cursor: null,
		focusedPill: null,
		pillSelect: null,
		filter: '',
	};
}

/** Send a library command while the destination's content is carried: any
 * other destination active means no command. `command` builds the wire shape
 * off the carried frame, so a library index the frame owns rides it. */
export function sendIfLoaded<Frame>(
	frame: Frame | null,
	command: (frame: Frame) => GuiLibraryCommand,
): void {
	if (frame === null) {
		return;
	}
	void sendLibraryCommand(command(frame));
}

/** The destination facts a pill-list controller works over: how the carried
 * frame builds its rows and pills, how the cursor moves over them, and how the
 * pill row's focus is stored. Every member stays a pure read of the frame or
 * the row — the command shapes stay in the destination's own module. */
export interface PillListConfig<
	Frame,
	Row extends GeometryRow,
	Item,
	Pill,
	Focus,
	PillEntry,
> {
	/** The carried content slice, or `null` while any other destination is
	 * active. */
	frame(): Frame | null;
	/** The list's scroll store (design D9): the native offset and the measured
	 * viewport the controller's scroll actions write. */
	scroll: Writable<ListScroll>;
	/** The painted rows a frame builds under a filter text (design D12). */
	buildRows(frame: Frame, filter: string): Row[];
	/** The item a painted row names, or `null` for a row the cursor skips — a
	 * letter heading names no item. */
	itemOf(row: Row): Item | null;
	/** The cursor's movement over the painted rows (design D7): the plain
	 * movement, or the destination's heading-skipping one. */
	move(
		rows: readonly Row[],
		cursor: string | null,
		move: ListMove,
		page?: number,
	): string | null;
	/** The frame's pill entries, in paint order. */
	pills(frame: Frame): readonly PillEntry[];
	/** The pill a pill entry carries: the set-pill command's payload. */
	pillOf(entry: PillEntry): Pill;
	/** The pill's stable key: the each-block's identity and the focus key. */
	pillKey(pill: Pill): string;
	/** The focus value a pill leaves in the state: the pill itself, or its
	 * key. */
	focusOf(pill: Pill): Focus;
	/** The focus value's key: a stale focus resolves against the carried
	 * pills by it, because every carried frame is a fresh deserialization and
	 * an object-identity find would lose the focus across frames (row 10.1
	 * review P2). */
	focusKey(focus: Focus): string;
}

/** The pill-list controller: the interaction-state store plus the actions
 * over it. The command sends stay in the destination's module — only the
 * select, which carries the shared Hero and menu reset, runs here. */
export interface PillListController<Frame, Row, Item, Pill, Focus> {
	/** The destination's interaction state store. */
	readonly ui: Writable<PillListUiState<Pill, Focus>>;
	/** The painted rows of the carried content (design D12). */
	rows(): Row[];
	/** The row the cursor rests on, or `null` while no painted row carries
	 * it: the keys read it to play the item or queue it. */
	cursorRow(): Item | null;
	/** Send the select command the core stores, whose frame paints the Hero,
	 * and reset the facts a new selection repaints: the Hero overlay and the
	 * menus. A select repaints the Narrow arrangement's Hero overlay and
	 * closes an open menu, because the facts they paint belong to the
	 * previous row, as a Music pick's do. */
	select(command: (frame: Frame) => GuiLibraryCommand): void;
	/** Narrow the list to the titles that contain `filter`, without case
	 * (design D7's filter rule): the cursor follows the rows. */
	setFilter(filter: string): void;
	/** Close the filter with Esc: the text goes with it, so a closed filter
	 * narrows nothing (design D3, step 2). The cursor follows the widened
	 * rows. */
	closeFilter(): void;
	/** Accept the filter with Enter: the field closes and the list keeps the
	 * text (design D3, step 2). */
	acceptFilter(): void;
	/** Move the list's cursor one movement along (design D12): Up and Down
	 * move a row, Page Up and Page Down move the rows in view, and Home and
	 * End take the list's ends. The scroll offset follows, so the cursor row
	 * stays fully visible. */
	moveCursor(move: ListMove): void;
	/** Report the list's measured viewport height (design D12): the rows in
	 * view follow it, and the scroll offset stays inside the narrower
	 * range. */
	measure(viewportHeight: number): void;
	/** Report the list's native scroll offset (design D12): a user scroll
	 * moves the rows in view, and the offset stays inside the list's
	 * range. */
	scroll(scrollTop: number): void;
	/** The pill the pill row's focus holds, resolved against the frame's
	 * carried pills so a stale pill focuses nothing: the pill the keys' Space
	 * and Enter select (design D6, D12). */
	focusedPill(frame: Frame): Pill | null;
	/** Move the focused pill one pill along the row (design D12): Right and
	 * Left walk the pills, and the first of them with no pill focused takes
	 * the end it walks from. */
	movePillFocus(step: number): void;
	/** Ask for the focused pill to be selected: Space and Enter request it
	 * (design D6, D12). */
	requestPillSelect(pill: Pill): void;
	/** Consume the keys' pill request, or `null` while none is pending: the
	 * intent is consumed here, so one key sends one command. */
	takePillSelect(): Pill | null;
}

/** Create one destination's pill-list controller (row 7C.1): the state store
 * and every action the keys and the list call, over the destination's frame,
 * rows, and pills. */
export function createPillList<
	Frame,
	Row extends GeometryRow,
	Item,
	Pill,
	Focus,
	PillEntry,
>(
	config: PillListConfig<Frame, Row, Item, Pill, Focus, PillEntry>,
): PillListController<Frame, Row, Item, Pill, Focus> {
	const ui = writable<PillListUiState<Pill, Focus>>(initialPillListUi());

	function rows(filter = get(ui).filter): Row[] {
		const frame = config.frame();
		return frame === null ? [] : config.buildRows(frame, filter);
	}

	/** Scroll the list so the cursor's row paints fully, in the scroll store
	 * (design D12, D9). */
	function followCursor(list: readonly Row[], cursor: string | null): void {
		config.scroll.update((scroll) => {
			const scrollTop = scrollTopToShowCursor(list, cursor, scroll.scrollTop, scroll.viewportHeight);
			return scrollTop === scroll.scrollTop ? scroll : { ...scroll, scrollTop };
		});
	}

	/** Settle the cursor once the painted rows may have narrowed (design D12): the
	 * cursor keeps its row while that row still paints, and falls to the first
	 * painted row otherwise. The scroll store then shows the cursor — the Music
	 * filter's own resolve (design D7). */
	function settleCursor(): void {
		const state = get(ui);
		const list = rows(state.filter);
		const cursor = list.some((row) => row.id === state.cursor)
			? state.cursor
			: config.move(list, null, 'home');
		if (cursor !== state.cursor) {
			ui.update((current) => ({ ...current, cursor }));
		}
		followCursor(list, cursor);
	}

	return {
		ui,

		rows,

		cursorRow() {
			const row = rows().find((candidate) => candidate.id === get(ui).cursor);
			return row === undefined ? null : config.itemOf(row);
		},

		select(command) {
			const frame = config.frame();
			if (frame === null) {
				return;
			}
			void sendLibraryCommand(command(frame));
			libraryUi.update((state) => ({
				...state,
				heroDismissed: false,
				itemMenuOpen: false,
				rowMenu: null,
			}));
		},

		setFilter(filter) {
			ui.update((state) => ({ ...state, filter }));
			settleCursor();
		},

		closeFilter() {
			libraryUi.update((state) => ({ ...state, filterOpen: false }));
			ui.update((state) => ({ ...state, filter: '' }));
			settleCursor();
		},

		acceptFilter() {
			libraryUi.update((state) => ({ ...state, filterOpen: false }));
		},

		moveCursor(move) {
			const list = rows();
			const current = get(ui);
			const scroll = get(config.scroll);
			const view = listWindow(list, scroll.scrollTop, scroll.viewportHeight);
			const cursor = config.move(list, current.cursor, move, view.end - view.start);
			if (cursor === current.cursor) {
				return;
			}
			ui.update((state) => ({ ...state, cursor }));
			followCursor(list, cursor);
		},

		measure(viewportHeight) {
			config.scroll.update((scroll) => ({
				...scroll,
				viewportHeight,
				scrollTop: clampScrollTop(rows(), scroll.scrollTop, viewportHeight),
			}));
		},

		scroll(scrollTop) {
			config.scroll.update((scroll) => {
				const top = clampScrollTop(rows(), Math.max(scrollTop, 0), scroll.viewportHeight);
				return top === scroll.scrollTop ? scroll : { ...scroll, scrollTop: top };
			});
		},

		focusedPill(frame) {
			const focus = get(ui).focusedPill;
			if (focus === null) {
				return null;
			}
			const key = config.focusKey(focus);
			const entry = config
				.pills(frame)
				.find((candidate) => config.pillKey(config.pillOf(candidate)) === key);
			return entry === undefined ? null : config.pillOf(entry);
		},

		movePillFocus(step) {
			const frame = config.frame();
			const entries = frame === null ? [] : config.pills(frame);
			const focus = get(ui).focusedPill;
			const entry = stepPillFocus(
				entries,
				focus === null ? null : config.focusKey(focus),
				(candidate) => config.pillKey(config.pillOf(candidate)),
				step,
			);
			if (entry !== null) {
				const pill = config.pillOf(entry);
				ui.update((state) => ({ ...state, focusedPill: config.focusOf(pill) }));
			}
		},

		requestPillSelect(pill) {
			ui.update((state) => ({ ...state, pillSelect: pill }));
		},

		takePillSelect() {
			const { pillSelect } = get(ui);
			if (pillSelect === null) {
				return null;
			}
			ui.update((state) => ({ ...state, pillSelect: null }));
			return pillSelect;
		},
	};
}
