// The Settings screen's own state (add-gui-settings-screen design D1, ADR
// 0037): what the window content owns and the core never reads — the keyboard
// pane, the sidebar and content cursors, the Mini pane (which is the keyboard
// pane), the form drafts, the choice marks before apply, and the capture.
// The core owns the open section and every section's data, so nothing here
// mirrors them: the screen paints the frame and keeps only the facts the
// user's own input moves.
//
// One pane fact serves both jobs: in Mini only the pane that holds the
// keyboard paints, so opening a section moves the keyboard to the content and
// Esc's content-to-sidebar step is what shows the sidebar again.

import { get, writable } from 'svelte/store';
import { sendSettingsCommand } from '$lib/bridge/settings';
import type {
	GuiChoicesKind,
	GuiFeedKind,
	GuiServiceActionIntent,
	GuiSettingsContent,
	GuiSettingsFeed,
	GuiSettingsFrame,
	GuiSettingsSection,
	GuiSettingsSetup,
	GuiTargetChoice,
} from '$lib/bridge/types';
import type { DomKey } from '$lib/bridge/types';
import type { SettingsEntry } from './entries';
import { settingsEntries } from './entries';

/** The pane the keyboard is on — and the one Mini paints (design D11). */
export type SettingsPane = 'sidebar' | 'content';

/** The form a focused text field belongs to (design D11, step 3): the fact
 * the key router reads before it types into the field. */
export type SettingsForm = 'embySetup' | 'audiobookshelfSetup' | 'feed' | 'rename';

/** The Emby setup form's fields, as the user typed them (design D9). The
 * password lives here until the submit carries it to the core; no frame
 * brings it back. */
export interface EmbySetupDraft {
	serverUrl: string;
	username: string;
	password: string;
}

/** The Audiobookshelf setup form's fields (design D9). */
export interface AudiobookshelfSetupDraft {
	serverUrl: string;
	apiKey: string;
}

/** The feed add or edit form's fields (design D9). `identity` is the
 * subscription URL an edit addresses; `null` on an add. */
export interface FeedFormDraft {
	identity: string | null;
	name: string;
	url: string;
	kind: GuiFeedKind;
}

/** The playlist rename field's draft (design D9). */
export interface RenameDraft {
	id: string;
	name: string;
}

/** The Settings screen's interaction state. */
export interface SettingsUiState {
	/** The pane the keyboard is on (design D11): `sidebar` or `content`. In
	 * Mini it is also the pane that paints. */
	pane: SettingsPane;
	/** The sidebar's cursor, an index into the frame's `sections`. */
	sidebarCursor: number;
	/** The content pane's cursor, an index into the section's entries. */
	contentCursor: number;
	/** The section the last frame showed: a change re-anchors the sidebar
	 * cursor and drops the content cursor and every draft. `null` before the
	 * first open frame. */
	section: GuiSettingsSection | null;
	/** The binding target a capture waits on (design D6), or `null`: the
	 * frame's own target grammar — an action id, `prefix:<action id>`, or
	 * `prefix`. `sent` marks the capture that sent its `keysCapture` and
	 * waits for the next frame's answer. */
	capture: { target: string; sent: boolean } | null;
	/** The marked ids of each touched choice list, by the row's setting
	 * name; a row the user has not marked yet falls back to the frame's
	 * `selected` flags. */
	choiceMarks: Record<string, string[]>;
	/** The form whose text field holds the keyboard (design D11, step 3), or
	 * `null`. A field's DOM focus events set and clear it. */
	textField: SettingsForm | null;
	/** The Emby setup draft; `null` while the frame opens no Emby form. */
	embySetup: EmbySetupDraft | null;
	/** The Audiobookshelf setup draft; `null` while the frame opens no
	 * Audiobookshelf form. */
	audiobookshelfSetup: AudiobookshelfSetupDraft | null;
	/** The feed add or edit draft; `null` while no feed form is open. */
	feedForm: FeedFormDraft | null;
	/** The playlist rename draft; `null` while no rename field is open. */
	rename: RenameDraft | null;
}

/** The state a freshly opened screen starts with. */
export function initialSettingsUi(): SettingsUiState {
	return {
		pane: 'sidebar',
		sidebarCursor: 0,
		contentCursor: 0,
		section: null,
		capture: null,
		choiceMarks: {},
		textField: null,
		embySetup: null,
		audiobookshelfSetup: null,
		feedForm: null,
		rename: null,
	};
}

/** The Settings screen's interaction state. */
export const settingsUi = writable<SettingsUiState>(initialSettingsUi());

/** The entries the section's content pane walks (design D11): the flat list
 * the cursor indexes and the components paint, with the choice marks the user
 * holds applied. */
export function currentSettingsEntries(
	content: GuiSettingsContent,
	ui: SettingsUiState = get(settingsUi),
): SettingsEntry[] {
	return settingsEntries(content, ui.choiceMarks);
}

/** The entry the content cursor rests on, or `null` while the section lists
 * none. */
export function focusedSettingsEntry(
	content: GuiSettingsContent,
	ui: SettingsUiState = get(settingsUi),
): SettingsEntry | null {
	const entries = currentSettingsEntries(content, ui);
	return entries[Math.min(ui.contentCursor, entries.length - 1)] ?? null;
}

/** The id of the entry the content cursor rests on, or `null` while the
 * section lists none: the fact a component's rows paint their cursor fill
 * from. */
export function cursorEntryId(
	entries: SettingsEntry[],
	ui: SettingsUiState = get(settingsUi),
): string | null {
	return entries[Math.min(ui.contentCursor, entries.length - 1)]?.id ?? null;
}

/** Activate the entry `id` names (a pointer press on its row): the same
 * activation Enter runs on the cursor entry. */
export function activateSettingsEntryId(entries: SettingsEntry[], id: string): void {
	const entry = entries.find((candidate) => candidate.id === id);
	if (entry !== undefined) {
		activateSettingsEntry(entry);
	}
}

/** A pointer press on an entry's row (design D11): the cursor moves onto it
 * and the keyboard takes the content pane, then the row's activation runs —
 * the one gesture a click always means. */
export function pressSettingsEntry(entries: SettingsEntry[], id: string): void {
	const index = entries.findIndex((candidate) => candidate.id === id);
	if (index === -1) {
		return;
	}
	settingsUi.update((ui) => ({ ...ui, pane: 'content', contentCursor: index }));
	activateSettingsEntry(entries[index]);
}

/** Open a section: the sidebar entry's activation (design D11). The command
 * starts the section's load in the core (design D9), and the keyboard moves
 * to the content so Mini shows it (the spec's *Narrow window* requirement). */
export function selectSettingsSection(section: GuiSettingsSection): void {
	void sendSettingsCommand({ command: 'selectSection', section });
	settingsUi.update((ui) => ({ ...ui, pane: 'content' }));
}

/** Switch the keyboard between the sidebar and the content (design D11): Tab
 * and Shift+Tab's one move. */
export function switchSettingsPane(): void {
	settingsUi.update((ui) => ({
		...ui,
		pane: ui.pane === 'sidebar' ? 'content' : 'sidebar',
	}));
}

/** Put the keyboard on one pane (a pointer press on it). */
export function focusSettingsPane(pane: SettingsPane): void {
	settingsUi.update((ui) => (ui.pane === pane ? ui : { ...ui, pane }));
}

/** The key map of one movement name to a step or an end. */
const CURSOR_STEPS = {
	up: -1,
	down: 1,
	pageUp: -10,
	pageDown: 10,
} as const;

/** Move the focused pane's cursor one movement along (design D11): Up and
 * Down a row, Page Up and Page Down a page of rows, Home and End the ends.
 * The cursor stays inside the list the pane paints. */
export function moveSettingsCursor(move: string, count: number): void {
	settingsUi.update((ui) => {
		if (count <= 0) {
			return ui;
		}
		const cursor = ui.pane === 'sidebar' ? ui.sidebarCursor : ui.contentCursor;
		let next: number;
		if (move === 'home') {
			next = 0;
		} else if (move === 'end') {
			next = count - 1;
		} else {
			const step = CURSOR_STEPS[move as keyof typeof CURSOR_STEPS] ?? 0;
			next = Math.min(Math.max(cursor + step, 0), count - 1);
		}
		return ui.pane === 'sidebar'
			? { ...ui, sidebarCursor: next }
			: { ...ui, contentCursor: next };
	});
}

/** Point the focused pane's cursor at an index (a pointer press on an entry
 * moves the cursor onto it before its activation). */
export function pointSettingsCursor(index: number): void {
	settingsUi.update((ui) =>
		ui.pane === 'sidebar'
			? { ...ui, sidebarCursor: index }
			: { ...ui, contentCursor: index },
	);
}

/** Esc's ladder while the screen shows (design D11, the spec's *Esc closes
 * the screen from the sidebar*): a capture, a form, or a draft ends first,
 * the content hands the keyboard back to the sidebar, and the sidebar's Esc
 * closes the screen. The capture's and the text field's own steps run before
 * this in the router, so an Esc that reaches this point faces no capture and
 * no field hold. */
export function settingsEscape(settings: GuiSettingsFrame): void {
	const ui = get(settingsUi);
	if (ui.feedForm !== null || ui.rename !== null) {
		settingsUi.update((state) => ({ ...state, feedForm: null, rename: null }));
		return;
	}
	const setup = settings.content.kind === 'services' ? settings.content.setup : null;
	if (setup !== null) {
		void sendSettingsCommand({ command: 'cancelSetup', service: setup.service });
		return;
	}
	if (ui.pane === 'content') {
		settingsUi.update((state) => ({ ...state, pane: 'sidebar' }));
		return;
	}
	void sendSettingsCommand({ command: 'close' });
}

/** Arm the capture for one binding target (design D6): the next key press is
 * its candidate chord. */
export function startKeysCapture(target: string): void {
	settingsUi.update((ui) => ({ ...ui, capture: { target, sent: false } }));
}

/** End the capture without a candidate: Esc's cancel (design D6). */
export function cancelKeysCapture(): void {
	settingsUi.update((ui) => (ui.capture === null ? ui : { ...ui, capture: null }));
}

/** Send the pressed key as the capture's candidate chord (design D6): the
 * raw DOM key rides `keysCapture`, and the capture stays painted until the
 * next frame answers it. */
export function sendKeysCapture(target: string, key: DomKey): void {
	void sendSettingsCommand({ command: 'keysCapture', target, key });
	settingsUi.update((ui) =>
		ui.capture === null ? ui : { ...ui, capture: { ...ui.capture, sent: true } },
	);
}

/** The marked ids of one choice-list row: the user's marks while they hold
 * any, else the frame's `selected` flags. */
export function markedChoices(
	row: { key: string; choices: { id: string; selected: boolean }[] | null },
	ui: SettingsUiState = get(settingsUi),
): string[] {
	return (
		ui.choiceMarks[row.key] ?? row.choices?.filter((c) => c.selected).map((c) => c.id) ?? []
	);
}

/** Flip one mark of a choice list (design D11's Space). */
export function toggleChoiceMark(setting: string, id: string, row: { choices: { id: string; selected: boolean }[] | null }): void {
	settingsUi.update((ui) => {
		const marked = markedChoices({ key: setting, choices: row.choices }, ui);
		const next = marked.includes(id)
			? marked.filter((candidate) => candidate !== id)
			: [...marked, id];
		return { ...ui, choiceMarks: { ...ui.choiceMarks, [setting]: next } };
	});
}

/** Apply a choice list's marked ids (design D9): the `applyChoices` command
 * runs the TUI's multiselect commit. */
export function applyChoiceList(kind: GuiChoicesKind, row: { key: string; choices: { id: string; selected: boolean }[] | null }): void {
	void sendSettingsCommand({
		command: 'applyChoices',
		kind,
		selected: markedChoices(row),
	});
}

/** The kind an `applyChoices` names for a row's setting: the wire names
 * coincide (`hiddenLibraries`, `feedViewLibraries`, `myLanguages`), so the
 * row's own key resolves it. `null` for any other row. */
export function choicesKindOf(key: string): GuiChoicesKind | null {
	return key === 'hiddenLibraries' || key === 'feedViewLibraries' || key === 'myLanguages'
		? key
		: null;
}

/** A form's text field holds the keyboard (the field's DOM `focus`): the key
 * router's step 3 gate. */
export function focusSettingsField(form: SettingsForm): void {
	settingsUi.update((ui) => (ui.textField === form ? ui : { ...ui, textField: form }));
}

/** A form's field lost the keyboard: the `focusout` whose target leaves the
 * form clears the hold. */
export function blurSettingsField(form: SettingsForm): void {
	settingsUi.update((ui) => (ui.textField === form ? { ...ui, textField: null } : ui));
}

/** Open the feed form: an add (`feed === null`) starts empty, an edit seeds
 * the subscription's name, URL, and kind with its URL as the identity. */
export function openFeedForm(feed: GuiSettingsFeed | null): void {
	settingsUi.update((ui) => ({
		...ui,
		feedForm:
			feed === null
				? { identity: null, name: '', url: '', kind: 'audio' }
				: { identity: feed.url, name: feed.name, url: feed.url, kind: feed.kind },
	}));
}

/** Change one feed-form field (the input's own event). */
export function setFeedForm(fields: Partial<Omit<FeedFormDraft, 'identity'>>): void {
	settingsUi.update((ui) =>
		ui.feedForm === null ? ui : { ...ui, feedForm: { ...ui.feedForm, ...fields } },
	);
}

/** Open the playlist rename field on `id`, seeded with the shown name. */
export function openRenameDraft(id: string, name: string): void {
	settingsUi.update((ui) => ({ ...ui, rename: { id, name } }));
}

/** Change the rename draft's text. */
export function setRenameDraft(name: string): void {
	settingsUi.update((ui) =>
		ui.rename === null ? ui : { ...ui, rename: { ...ui.rename, name } },
	);
}

/** Change one Emby setup field (the input's own event). */
export function setEmbySetupDraft(fields: Partial<EmbySetupDraft>): void {
	settingsUi.update((ui) =>
		ui.embySetup === null ? ui : { ...ui, embySetup: { ...ui.embySetup, ...fields } },
	);
}

/** Change one Audiobookshelf setup field. */
export function setAudiobookshelfSetupDraft(fields: Partial<AudiobookshelfSetupDraft>): void {
	settingsUi.update((ui) =>
		ui.audiobookshelfSetup === null
			? ui
			: { ...ui, audiobookshelfSetup: { ...ui.audiobookshelfSetup, ...fields } },
	);
}

/** Submit the form that holds the keyboard (design D11, step 3): the typed
 * secret rides the command and its field clears at once, so no draft keeps
 * it past the send. A setup submit while the form is busy sends nothing. */
export function submitSettingsForm(content: GuiSettingsContent): void {
	const ui = get(settingsUi);
	switch (ui.textField) {
		case 'embySetup': {
			if (ui.embySetup === null || setupBusy(content, 'emby')) {
				return;
			}
			void sendSettingsCommand({ command: 'embySetup', ...ui.embySetup });
			settingsUi.update((state) =>
				state.embySetup === null
					? state
					: { ...state, embySetup: { ...state.embySetup, password: '' } },
			);
			return;
		}
		case 'audiobookshelfSetup': {
			if (ui.audiobookshelfSetup === null || setupBusy(content, 'audiobookshelf')) {
				return;
			}
			void sendSettingsCommand({ command: 'audiobookshelfSetup', ...ui.audiobookshelfSetup });
			settingsUi.update((state) =>
				state.audiobookshelfSetup === null
					? state
					: { ...state, audiobookshelfSetup: { ...state.audiobookshelfSetup, apiKey: '' } },
			);
			return;
		}
		case 'feed': {
			const form = ui.feedForm;
			if (form === null) {
				return;
			}
			if (form.identity === null) {
				void sendSettingsCommand({
					command: 'feedAdd',
					name: form.name,
					url: form.url,
					kind: form.kind,
				});
			} else {
				void sendSettingsCommand({
					command: 'feedEdit',
					url: form.identity,
					name: form.name,
					kind: form.kind,
				});
			}
			settingsUi.update((state) => ({ ...state, feedForm: null, textField: null }));
			return;
		}
		case 'rename': {
			const draft = ui.rename;
			if (draft === null) {
				return;
			}
			void sendSettingsCommand({
				command: 'playlistRename',
				id: draft.id,
				name: draft.name,
			});
			settingsUi.update((state) => ({ ...state, rename: null, textField: null }));
			return;
		}
		case null:
			return;
	}
}

/** Whether the named setup form is busy, so a submit under a running
 * validation sends nothing. */
function setupBusy(content: GuiSettingsContent, service: 'emby' | 'audiobookshelf'): boolean {
	const setup: GuiSettingsSetup | null =
		content.kind === 'services' ? content.setup : null;
	return setup !== null && setup.service === service && setup.busy;
}

/** Cancel the form that holds the keyboard (design D11, step 3): Esc in a
 * focused field. The setup forms cancel in the core, which retracts the
 * form; the front-end drafts simply close. */
export function cancelSettingsForm(): void {
	const ui = get(settingsUi);
	switch (ui.textField) {
		case 'embySetup':
			void sendSettingsCommand({ command: 'cancelSetup', service: 'emby' });
			return;
		case 'audiobookshelfSetup':
			void sendSettingsCommand({ command: 'cancelSetup', service: 'audiobookshelf' });
			return;
		case 'feed':
		case 'rename':
			settingsUi.update((state) => ({
				...state,
				feedForm: null,
				rename: null,
				textField: null,
			}));
			return;
		case null:
			return;
	}
}

/** Run one Service action (a button's press or the cursor's activation). */
export function runServiceAction(intent: GuiServiceActionIntent): void {
	void sendSettingsCommand({ command: 'serviceAction', action: intent });
}

/** Select a playback target in the Sessions section: the chooser's own path,
 * so the screen stays open (the spec's *Sessions* requirement). */
export function selectSettingsTarget(target: GuiTargetChoice): void {
	void sendSettingsCommand({ command: 'selectTarget', target: target.key });
}

/** Activate one content entry (design D11's Enter, and Space on a toggle or
 * a choice mark): the cursor row's own command, or the edit the row opens. */
export function activateSettingsEntry(entry: SettingsEntry): void {
	switch (entry.kind) {
		case 'setting': {
			const { row } = entry;
			if (row.kind === 'action' && row.key === 'resetUiState') {
				// The core's activate of this row is a no-op (its TUI activation
				// runs through the shell's own path), so the GUI's row sends the
				// reset command (design D5).
				void sendSettingsCommand({ command: 'resetUiState' });
			} else if (row.kind !== 'choiceList') {
				// A choice list's activate would raise the TUI's multiselect,
				// which the overlay drain drops; its marks edit in place.
				void sendSettingsCommand({ command: 'activate', setting: row.key });
			}
			return;
		}
		case 'choice':
			toggleChoiceMark(entry.setting, entry.choice.id, { choices: entry.choices });
			return;
		case 'choicesApply':
			applyChoiceList(entry.setting, entry.row);
			return;
		case 'route':
			void sendSettingsCommand({ command: 'routeDevices', library: entry.route.library });
			return;
		case 'routeClear':
			void sendSettingsCommand({ command: 'assignRoute', library: entry.library, endpoint: null });
			return;
		case 'routeDevice':
			// A device with no endpoint is not currently routable; the row
			// paints disabled and a press on it assigns nothing, so the GUI
			// never asks the core to clear a route by accident.
			if (entry.device.endpoint !== null) {
				void sendSettingsCommand({
					command: 'assignRoute',
					library: entry.library,
					endpoint: entry.device.endpoint,
				});
			}
			return;
		case 'serviceAction':
			runServiceAction(entry.intent);
			return;
		case 'feedAdd':
			openFeedForm(null);
			return;
		case 'feed':
			openFeedForm(entry.feed);
			return;
		case 'feedRemove':
			void sendSettingsCommand({ command: 'feedRemove', url: entry.feed.url });
			return;
		case 'keyTarget':
			startKeysCapture(entry.target);
			return;
		case 'keyReset':
			void sendSettingsCommand({ command: 'keysReset', action: entry.action });
			return;
		case 'keyClear':
			void sendSettingsCommand({ command: 'keysClear', target: entry.target });
			return;
		case 'target':
			selectSettingsTarget(entry.target);
			return;
		case 'playlist':
			void sendSettingsCommand({ command: 'playlistOpen', id: entry.playlist.id });
			return;
		case 'playlistAction':
			void sendSettingsCommand({
				command: 'playlistAction',
				id: entry.playlistId,
				action: entry.action,
			});
			return;
		case 'playlistItemAction':
			void sendSettingsCommand({
				command: 'playlistItemAction',
				id: entry.playlistId,
				itemId: entry.itemId,
				action: entry.action,
			});
			return;
		case 'playlistRename':
			openRenameDraft(entry.playlistId, entry.name);
			return;
		case 'playlistDelete':
			void sendSettingsCommand({ command: 'playlistDelete', id: entry.playlistId });
			return;
		case 'playlistBack':
			void sendSettingsCommand({ command: 'playlistBack' });
			return;
		case 'playlistRefresh':
			void sendSettingsCommand({ command: 'playlistRefresh', id: entry.playlistId });
			return;
	}
}

/** Whether Space activates the focused content entry (design D11): a
 * toggle's flip or a choice mark's toggle. Every other entry leaves Space to
 * the core, where it plays or pauses (the spec's *Global keys keep working
 * behind the screen*). */
export function spaceActivates(entry: SettingsEntry | null): boolean {
	return (
		(entry?.kind === 'setting' && entry.row.kind === 'toggle') || entry?.kind === 'choice'
	);
}

/** Fold one arriving frame's Settings section into the interaction state:
 * the screen's open resets everything (a reopen starts on the sidebar at the
 * frame's section), a section change re-anchors the sidebar cursor and moves
 * the keyboard to the content — opening a section shows it (the spec's
 * *Narrow window* requirement) — and the frame's answer ends a sent capture.
 * The setup draft seeds from the form's carried server URL the first time
 * the frame opens it, and leaves with the form. `null` closes the screen's
 * state whole. */
export function syncSettingsFrame(settings: GuiSettingsFrame | null): void {
	if (settings === null) {
		settingsUi.set(initialSettingsUi());
		return;
	}
	settingsUi.update((ui) => {
		let next = ui;
		if (ui.section !== settings.section) {
			const sidebarCursor = settings.sections.indexOf(settings.section);
			const opened = ui.section === null;
			next = {
				...initialSettingsUi(),
				// A fresh open lands the keyboard on the sidebar; a section
				// change while the screen is open shows the new section.
				pane: opened ? 'sidebar' : 'content',
				sidebarCursor: Math.max(0, sidebarCursor),
				section: settings.section,
			};
		}
		// The frame answered the sent capture: the new chord or the refusal
		// paints where the prompt was.
		if (next.capture?.sent) {
			next = { ...next, capture: null };
		}
		const setup = settings.content.kind === 'services' ? settings.content.setup : null;
		if (setup === null) {
			if (
				next.textField === 'embySetup' ||
				next.textField === 'audiobookshelfSetup'
			) {
				next = { ...next, textField: null };
			}
			next = { ...next, embySetup: null, audiobookshelfSetup: null };
		} else if (setup.service === 'emby' && next.embySetup === null) {
			next = {
				...next,
				embySetup: { serverUrl: setup.serverUrl, username: '', password: '' },
			};
		} else if (setup.service === 'audiobookshelf' && next.audiobookshelfSetup === null) {
			next = {
				...next,
				audiobookshelfSetup: { serverUrl: setup.serverUrl, apiKey: '' },
			};
		}
		return next;
	});
}

/** Reset the interaction state whole (design D10): a new `uiResetEpoch`
 * drops the cursors, the drafts, the marks, and the capture while the core
 * keeps the open section. */
export function resetSettingsUi(): void {
	settingsUi.update((ui) => ({ ...initialSettingsUi(), section: ui.section }));
}
