// The Settings wire types of the GUI bridge contract (openspec change
// add-gui-settings-screen, design D5 and D9). Each type mirrors one serde DTO
// in crates/mbv-gui/src/bridge/ field for field: the names are the serde wire
// names, so a rename on the Rust side is a rename here. The `DomKey` the
// `keysCapture` command carries is the `key` command's own type in `types.ts`.

import type { DomKey, GuiTargetChoice, GuiTargetKey } from './types';

/** The Settings screen's frame section (design D9). `null` on `GuiFrame` while
 * the screen is closed. */
export interface GuiSettingsFrame {
	/** The section the window shows. */
	section: GuiSettingsSection;
	/** The sections the sidebar lists, in order (design D3). */
	sections: GuiSettingsSection[];
	/** The shown section's content. */
	content: GuiSettingsContent;
}

/** One Settings section, in sidebar order (design D3). */
export type GuiSettingsSection =
	| 'services'
	| 'playback'
	| 'display'
	| 'session'
	| 'library'
	| 'queue'
	| 'mpv'
	| 'feeds'
	| 'actions'
	| 'keys'
	| 'sessions'
	| 'playlists';

/** The shown section's content (design D9): one variant per kind of section. */
export type GuiSettingsContent =
	| { kind: 'rows'; rows: GuiSettingRow[] }
	| {
			/** The Library section (design D9): its choice rows, the routable
			 * libraries, the selected library, and its candidate devices. */
			kind: 'library';
			rows: GuiSettingRow[];
			routes: GuiSettingsRoute[] | null;
			selected: string | null;
			devices: GuiSettingsRouteDevice[] | null;
	  }
	| {
			kind: 'services';
			services: GuiSettingsService[];
			setup: GuiSettingsSetup | null;
	  }
	| {
			kind: 'feeds';
			subscriptions: GuiSettingsFeed[];
			adding: boolean;
	  }
	| {
			kind: 'keys';
			prefix: string | null;
			rows: GuiSettingsKeyRow[];
			refusal: GuiSettingsKeyRefusal | null;
	  }
	| { kind: 'sessions'; targets: GuiTargetChoice[] }
	| {
			kind: 'playlists';
			playlists: GuiPlaylistRow[];
			open: GuiOpenPlaylist | null;
			loading: boolean;
			openLoading: boolean;
	  };

/** One F2 row (design D9). */
export interface GuiSettingRow {
	/** The setting's stable wire name: the `activate` command's `setting`. */
	key: string;
	label: string;
	kind: GuiSettingRowKind;
	/** The value's text, as the TUI paints it. */
	value: string;
	/** The toggle's flag; `null` for a row that is no toggle. */
	flag: boolean | null;
	/** A choice list's choices once its load lands (row 5.1). */
	choices: GuiSettingChoice[] | null;
	/** The kind this row's `applyChoices` names (design D11 *Choice kind*):
	 * read here, never derived from `key`. `null` for a row that is no choice
	 * list. */
	choicesKind: GuiChoicesKind | null;
	/** The values a pick row lists (design D13); `null` for a row that is no
	 * pick. The `setValue` command accepts only one of these. */
	values: string[] | null;
}

/** How an F2 row is edited (design D9, D13). */
export type GuiSettingRowKind =
	| 'toggle'
	| 'cycle'
	| 'pick'
	| 'choiceList'
	| 'action';

/** One choice of a choice-list row (design D9): `id` is the value the apply
 * command sends back. */
export interface GuiSettingChoice {
	id: string;
	label: string;
	selected: boolean;
}

/** One routable library of the Library section's routes list (design D9):
 * `library` is the lowercased name the `routeDevices` and `assignRoute`
 * commands carry. */
export interface GuiSettingsRoute {
	library: string;
	/** The library's display name, as the TUI paints it. */
	name: string;
	/** The assigned device route; `null` while the library uses the
	 * automatic route. */
	endpoint: string | null;
}

/** One candidate device of the selected library (design D9): `name` is the
 * device, `endpoint` the route `assignRoute` sends; `null` while the device
 * is not currently routable. */
export interface GuiSettingsRouteDevice {
	name: string;
	endpoint: string | null;
}

/** One Service row (design D9). */
export interface GuiSettingsService {
	entry: GuiServiceEntry;
	name: string;
	/** The Service's state label, as the TUI paints it. */
	state: string;
	/** The Service's context line (the feed subscription count, or empty). */
	context: string;
	actions: GuiServiceAction[];
}

/** The three Services (design D9). */
export type GuiServiceEntry = 'emby' | 'audiobookshelf' | 'feeds';

/** One action a Service row offers (design D9). */
export interface GuiServiceAction {
	/** The intent the `serviceAction` command carries. */
	intent: GuiServiceActionIntent;
	label: string;
}

/** One Service action the window content can run (design D5). */
export type GuiServiceActionIntent =
	| 'setupEmby'
	| 'retryEmby'
	| 'repairEmby'
	| 'removeEmby'
	| 'setupAudiobookshelf'
	| 'testAudiobookshelf'
	| 'removeAudiobookshelf'
	| 'replaceAudiobookshelf'
	| 'manageFeeds';

/** The open setup form's facts (design D9). It carries no secret: the typed
 * password and API key stay in the core. */
export interface GuiSettingsSetup {
	service: GuiSetupService;
	serverUrl: string;
	busy: boolean;
	error: string;
}

/** The setup form the `cancelSetup` command names (design D5). */
export type GuiSetupService = 'emby' | 'audiobookshelf';

/** One feed subscription (design D9). */
export interface GuiSettingsFeed {
	name: string;
	/** The subscription's only stable identity: the `feedEdit` and
	 * `feedRemove` commands address it. */
	url: string;
	kind: GuiFeedKind;
}

/** A subscription's default media kind (design D9). */
export type GuiFeedKind = 'audio' | 'video';

/** One Keys row (design D9). */
export interface GuiSettingsKeyRow {
	/** The action's stable id: the `keysReset` command's `action`. */
	id: string;
	label: string;
	/** The action's declared section name. */
	section: string;
	/** The chords that fire the action under the loaded configuration. */
	chords: string[];
	/** The action's prefix-namespace chord, where one is assigned. */
	prefixChord: string | null;
	/** Whether the binding differs from the action's declared default. */
	changed: boolean;
	/** Whether the prefix namespace can address the action. */
	prefixAddressable: boolean;
}

/** A refused Keys edit (design D9). */
export interface GuiSettingsKeyRefusal {
	action: string;
	text: string;
}

/** One saved playlist (design D9). */
export interface GuiPlaylistRow {
	/** The playlist's stable Emby id: every playlist command addresses it. */
	id: string;
	name: string;
	itemCount: number;
}

/** The open playlist and its items (design D9). */
export interface GuiOpenPlaylist {
	id: string;
	name: string;
	items: GuiPlaylistItem[];
}

/** One item of the open playlist (design D9). */
export interface GuiPlaylistItem {
	id: string;
	name: string;
}

/** One playback-target choice (design D4), the chooser's own row DTO: the
 * Sessions section lists the same choices, so the section and the chooser
 * cannot disagree inside one frame. */
export type { GuiTargetChoice, GuiTargetKey } from './types';

/** One request from the Settings screen's window content (design D5), the wire
 * shape of `GuiSettingsCommand` in bridge/settings.rs. Identities are stable —
 * a setting name, a playlist id, a feed URL, a chord — never row positions. */
export type GuiSettingsCommand =
	| { command: 'close' }
	| { command: 'selectSection'; section: GuiSettingsSection }
	| {
			/** Activate an F2 row: `setting` is the row's stable setting name. */
			command: 'activate';
			setting: string;
	  }
	| {
			/** Pick one value for a value setting: `value` is one entry of the
			 * row's projected list (design D13). */
			command: 'setValue';
			setting: string;
			value: string;
	  }
	| {
			/** Apply a choice list's marked ids. */
			command: 'applyChoices';
			kind: GuiChoicesKind;
			selected: string[];
	  }
	| {
			/** Assign a library route, or clear it when `endpoint` is absent. */
			command: 'assignRoute';
			library: string;
			endpoint: string | null;
	  }
	| {
			/** Load the candidate devices of `library` for the Library
			 * section's routes list (row 5.1). */
			command: 'routeDevices';
			library: string;
	  }
	| {
			command: 'embySetup';
			serverUrl: string;
			username: string;
			password: string;
	  }
	| {
			command: 'audiobookshelfSetup';
			serverUrl: string;
			apiKey: string;
	  }
	| { command: 'serviceAction'; action: GuiServiceActionIntent }
	| { command: 'cancelSetup'; service: GuiSetupService }
	| { command: 'feedAdd'; name: string; url: string; kind: GuiFeedKind }
	| { command: 'feedEdit'; url: string; name: string; kind: GuiFeedKind }
	| { command: 'feedRemove'; url: string }
	| {
			/** Capture a key press for a binding target: the target's stable
			 * name and the raw DOM key. */
			command: 'keysCapture';
			target: string;
			key: DomKey;
	  }
	| { command: 'keysReset'; action: string }
	| { command: 'keysClear'; target: string }
	| { command: 'selectTarget'; target: GuiTargetKey }
	| { command: 'playlistOpen'; id: string | null }
	| { command: 'playlistBack' }
	| { command: 'playlistRefresh'; id: string | null }
	| {
			command: 'playlistAction';
			id: string;
			action: GuiPlaylistAction;
	  }
	| {
			command: 'playlistItemAction';
			id: string;
			itemId: string;
			action: GuiPlaylistAction;
	  }
	| { command: 'playlistRename'; id: string; name: string }
	| { command: 'playlistDelete'; id: string }
	| { command: 'resetUiState' };

/** One choice-list kind the `applyChoices` command names (design D5). */
export type GuiChoicesKind =
	| 'hiddenLibraries'
	| 'feedViewLibraries'
	| 'myLanguages';

/** The playlist operation a playlist command carries (design D5). */
export type GuiPlaylistAction = 'play' | 'shuffle' | 'enqueue';
