// The Settings screen's content entries (add-gui-settings-screen design
// D11): the flat, ordered list of activatable controls the content cursor
// walks. Each entry carries the stable identity its command needs — a
// setting name, a choice id, a library name, a feed URL, a playlist id —
// never a row position.
//
// The `entryId` builders are the one grammar for an entry's `id`: the
// section components paint their own structured rows and mark the row under
// the cursor by resolving the same id, so the keyboard's order and the
// paint's order cannot drift.

import type {
	GuiChoicesKind,
	GuiPlaylistAction,
	GuiPlaylistRow,
	GuiSettingChoice,
	GuiSettingRow,
	GuiSettingsContent,
	GuiSettingsFeed,
	GuiSettingsRoute,
	GuiSettingsRouteDevice,
	GuiServiceActionIntent,
	GuiServiceEntry,
	GuiTargetChoice,
} from '$lib/bridge/types';

/** The one grammar for an entry's `id`: the entries builder and every
 * section component name the same control with it. */
export const entryId = {
	setting: (key: string) => `setting:${key}`,
	choice: (key: string, id: string) => `choice:${key}:${id}`,
	choicesApply: (key: string) => `apply:${key}`,
	route: (library: string) => `route:${library}`,
	routeClear: (library: string) => `routeClear:${library}`,
	routeDevice: (library: string, name: string) => `routeDevice:${library}:${name}`,
	serviceAction: (entry: GuiServiceEntry, intent: GuiServiceActionIntent) =>
		`service:${entry}:${intent}`,
	feedAdd: () => 'feedAdd',
	feed: (url: string) => `feed:${url}`,
	feedRemove: (url: string) => `feedRemove:${url}`,
	keyTarget: (target: string) => `keyTarget:${target}`,
	keyReset: (action: string) => `keyReset:${action}`,
	keyClear: (target: string) => `keyClear:${target}`,
	target: (index: number) => `target:${index}`,
	playlist: (id: string) => `playlist:${id}`,
	playlistAction: (id: string, action: GuiPlaylistAction) =>
		`playlistAction:${id}:${action}`,
	playlistItemAction: (id: string, itemId: string, action: GuiPlaylistAction) =>
		`item:${id}:${itemId}:${action}`,
	playlistRename: (id: string) => `playlistRename:${id}`,
	playlistDelete: (id: string) => `playlistDelete:${id}`,
	playlistBack: () => 'playlistBack',
	playlistRefresh: (id: string | null) => `playlistRefresh:${id ?? 'list'}`,
} as const;

/** One activatable control of the content pane. */
export type SettingsEntry =
	| {
			kind: 'setting';
			id: string;
			row: GuiSettingRow;
	  }
	| {
			/** One mark of a choice list: Space toggles it. */
			kind: 'choice';
			id: string;
			/** The `applyChoices` kind the row resolves to. */
			setting: GuiChoicesKind;
			choice: GuiSettingChoice;
			/** The row's full choice list, which seeds the marks. */
			choices: GuiSettingChoice[] | null;
			/** Whether the user (or the frame) marks this choice. */
			marked: boolean;
	  }
	| {
			/** A choice list's Apply control. */
			kind: 'choicesApply';
			id: string;
			setting: GuiChoicesKind;
			row: GuiSettingRow;
	  }
	| {
			/** One routable library: selecting it loads its devices. */
			kind: 'route';
			id: string;
			route: GuiSettingsRoute;
	  }
	| {
			/** The routes list's automatic entry: assigning it clears the
			 * selected library's route. */
			kind: 'routeClear';
			id: string;
			library: string;
	  }
	| {
			/** One candidate device of the selected library. */
			kind: 'routeDevice';
			id: string;
			library: string;
			device: GuiSettingsRouteDevice;
	  }
	| {
			/** One Service action button. */
			kind: 'serviceAction';
			id: string;
			intent: GuiServiceActionIntent;
			label: string;
	  }
	| {
			/** The feed section's add control. */
			kind: 'feedAdd';
			id: string;
	  }
	| {
			/** One feed subscription: Enter opens its edit form. */
			kind: 'feed';
			id: string;
			feed: GuiSettingsFeed;
	  }
	| {
			/** One feed subscription's remove control. */
			kind: 'feedRemove';
			id: string;
			feed: GuiSettingsFeed;
	  }
	| {
			/** One Keys binding target: activating it arms the capture. */
			kind: 'keyTarget';
			id: string;
			/** The frame's target grammar: an action id, `prefix:<action
			 * id>`, or `prefix`. */
			target: string;
	  }
	| {
			/** One action's reset-to-default control. */
			kind: 'keyReset';
			id: string;
			action: string;
	  }
	| {
			/** One prefix-namespace clear control (a prefix assignment or the
			 * prefix chord). */
			kind: 'keyClear';
			id: string;
			target: string;
	  }
	| {
			/** One playback target of the Sessions section. */
			kind: 'target';
			id: string;
			target: GuiTargetChoice;
	  }
	| {
			/** One saved playlist: Enter opens it. */
			kind: 'playlist';
			id: string;
			playlist: GuiPlaylistRow;
	  }
	| {
			/** One action on a whole playlist: play, shuffle, or add. */
			kind: 'playlistAction';
			id: string;
			playlistId: string;
			action: GuiPlaylistAction;
	  }
	| {
			/** One action on one playlist item: play, shuffle, or add. */
			kind: 'playlistItemAction';
			id: string;
			playlistId: string;
			itemId: string;
			action: GuiPlaylistAction;
	  }
	| {
			/** A playlist's rename control: opens the rename field. */
			kind: 'playlistRename';
			id: string;
			playlistId: string;
			name: string;
	  }
	| {
			/** A playlist's delete control: raises the confirm. */
			kind: 'playlistDelete';
			id: string;
			playlistId: string;
	  }
	| {
			/** The open playlist's back control. */
			kind: 'playlistBack';
			id: string;
	  }
	| {
			/** A refresh control: the list's (`null`) or the open
			 * playlist's. */
			kind: 'playlistRefresh';
			id: string;
			playlistId: string | null;
	  };

/** The marked ids a row's choice list holds, seeded from the frame's
 * `selected` flags and overridden by the user's marks. */
function markedIds(
	row: GuiSettingRow,
	marks: Record<string, string[]>,
): Record<string, boolean> {
	const ids = marks[row.key] ?? row.choices?.filter((c) => c.selected).map((c) => c.id) ?? [];
	return Object.fromEntries(ids.map((id) => [id, true]));
}

/** The kind an `applyChoices` names for a choice-list row: the wire names
 * coincide (`hiddenLibraries`, `feedViewLibraries`, `myLanguages`). */
function choicesKindOf(key: string): GuiChoicesKind | null {
	return key === 'hiddenLibraries' || key === 'feedViewLibraries' || key === 'myLanguages'
		? key
		: null;
}

/** The entries one F2 row produces: a toggle, cycle, or action row is one
 * entry; a choice list is its marks plus its Apply control, painted under
 * the row's own header line. */
function rowEntries(row: GuiSettingRow, marks: Record<string, string[]>): SettingsEntry[] {
	if (row.kind !== 'choiceList') {
		return [{ kind: 'setting', id: entryId.setting(row.key), row }];
	}
	const kind = choicesKindOf(row.key);
	if (kind === null) {
		return [];
	}
	const marked = markedIds(row, marks);
	const entries: SettingsEntry[] = (row.choices ?? []).map((choice) => ({
		kind: 'choice',
		id: entryId.choice(row.key, choice.id),
		setting: kind,
		choice,
		choices: row.choices,
		marked: marked[choice.id] === true,
	}));
	entries.push({ kind: 'choicesApply', id: entryId.choicesApply(row.key), setting: kind, row });
	return entries;
}

/** The playlist row's five action entries (design D5): the row itself plus
 * play, shuffle, add, rename, and delete. */
function playlistEntries(playlist: GuiPlaylistRow): SettingsEntry[] {
	return [
		{ kind: 'playlist', id: entryId.playlist(playlist.id), playlist },
		{
			kind: 'playlistAction',
			id: entryId.playlistAction(playlist.id, 'play'),
			playlistId: playlist.id,
			action: 'play',
		},
		{
			kind: 'playlistAction',
			id: entryId.playlistAction(playlist.id, 'shuffle'),
			playlistId: playlist.id,
			action: 'shuffle',
		},
		{
			kind: 'playlistAction',
			id: entryId.playlistAction(playlist.id, 'enqueue'),
			playlistId: playlist.id,
			action: 'enqueue',
		},
		{
			kind: 'playlistRename',
			id: entryId.playlistRename(playlist.id),
			playlistId: playlist.id,
			name: playlist.name,
		},
		{
			kind: 'playlistDelete',
			id: entryId.playlistDelete(playlist.id),
			playlistId: playlist.id,
		},
	];
}

/** The entries one section's content produces, in paint order. `marks` is
 * the window content's held choice marks. */
export function settingsEntries(
	content: GuiSettingsContent,
	marks: Record<string, string[]>,
): SettingsEntry[] {
	switch (content.kind) {
		case 'rows':
			return content.rows.flatMap((row) => rowEntries(row, marks));
		case 'library': {
			const entries = content.rows.flatMap((row) => rowEntries(row, marks));
			for (const route of content.routes ?? []) {
				entries.push({ kind: 'route', id: entryId.route(route.library), route });
			}
			if (content.selected !== null) {
				const library = content.selected;
				entries.push({
					kind: 'routeClear',
					id: entryId.routeClear(library),
					library,
				});
				for (const device of content.devices ?? []) {
					entries.push({
						kind: 'routeDevice',
						id: entryId.routeDevice(library, device.name),
						library,
						device,
					});
				}
			}
			return entries;
		}
		case 'services':
			return content.services.flatMap((service) =>
				service.actions.map((action) => ({
					kind: 'serviceAction' as const,
					id: entryId.serviceAction(service.entry, action.intent),
					intent: action.intent,
					label: action.label,
				})),
			);
		case 'feeds': {
			const entries: SettingsEntry[] = [{ kind: 'feedAdd', id: entryId.feedAdd() }];
			for (const feed of content.subscriptions) {
				entries.push({ kind: 'feed', id: entryId.feed(feed.url), feed });
				entries.push({ kind: 'feedRemove', id: entryId.feedRemove(feed.url), feed });
			}
			return entries;
		}
		case 'keys': {
			const entries: SettingsEntry[] = [
				{ kind: 'keyTarget', id: entryId.keyTarget('prefix'), target: 'prefix' },
			];
			if (content.prefix !== null) {
				entries.push({ kind: 'keyClear', id: entryId.keyClear('prefix'), target: 'prefix' });
			}
			for (const row of content.rows) {
				entries.push({ kind: 'keyTarget', id: entryId.keyTarget(row.id), target: row.id });
				if (row.prefixAddressable) {
					entries.push({
						kind: 'keyTarget',
						id: entryId.keyTarget(`prefix:${row.id}`),
						target: `prefix:${row.id}`,
					});
				}
				if (row.changed) {
					entries.push({
						kind: 'keyReset',
						id: entryId.keyReset(row.id),
						action: row.id,
					});
				}
				if (row.prefixChord !== null) {
					entries.push({
						kind: 'keyClear',
						id: entryId.keyClear(`prefix:${row.id}`),
						target: `prefix:${row.id}`,
					});
				}
			}
			return entries;
		}
		case 'sessions':
			return content.targets.map((target, index) => ({
				kind: 'target',
				id: entryId.target(index),
				target,
			}));
		case 'playlists': {
			const entries: SettingsEntry[] = [];
			if (content.open === null) {
				entries.push({
					kind: 'playlistRefresh',
					id: entryId.playlistRefresh(null),
					playlistId: null,
				});
				for (const playlist of content.playlists) {
					entries.push(...playlistEntries(playlist));
				}
				return entries;
			}
			const open = content.open;
			entries.push(
				{ kind: 'playlistBack', id: entryId.playlistBack() },
				{
					kind: 'playlistRefresh',
					id: entryId.playlistRefresh(open.id),
					playlistId: open.id,
				},
				...playlistEntries({ id: open.id, name: open.name, itemCount: open.items.length }).slice(1),
			);
			for (const item of open.items) {
				for (const action of ['play', 'shuffle', 'enqueue'] as const) {
					entries.push({
						kind: 'playlistItemAction',
						id: entryId.playlistItemAction(open.id, item.id, action),
						playlistId: open.id,
						itemId: item.id,
						action,
					});
				}
			}
			return entries;
		}
	}
}
