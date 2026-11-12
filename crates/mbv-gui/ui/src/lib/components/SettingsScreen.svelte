<script lang="ts">
	// The Settings screen (add-gui-settings-screen design D11, D13, ADR 0037):
	// the sidebar in the Queue column's place and the shown section's content in
	// the Library column's place, with the status line closing the screen.
	// The sidebar is the canvas's fixed 260 px, independent of the Queue
	// column's dragged width and its collapse toggle, and paints no divider.
	// Below the `libraryArrangement` Mini threshold (measured as the window
	// width minus that width) only the pane that holds the keyboard shows.
	// The content is the canvas `main`: 32 px by 48 px padding, a 1120 px
	// maximum width, and the section's name as its 24 px title.
	// The root carries `min-w-0 flex-1`: `App.svelte` gives it a row of a
	// flex container, so without them the screen shrinks to its content
	// width instead of taking the canvas (live finding, Settings screen).
	import type { GuiArrangement } from '$lib/bridge/arrangement';
	import { SETTINGS_SIDEBAR_WIDTH_PX } from '$lib/bridge/arrangement';
	import type { GuiSettingsFrame, GuiSettingsSection, GuiStatusBar } from '$lib/bridge/types';
	import {
		currentSettingsEntries,
		cursorEntryId,
		hideSettingsCursor,
		settingsUi,
	} from '$lib/settings/state';
	import SettingsSidebar from './SettingsSidebar.svelte';
	import SettingsStatusBar from './SettingsStatusBar.svelte';
	import SettingsRows from './SettingsRows.svelte';
	import SettingsLibrary from './SettingsLibrary.svelte';
	import SettingsServices from './SettingsServices.svelte';
	import SettingsFeeds from './SettingsFeeds.svelte';
	import SettingsKeys from './SettingsKeys.svelte';
	import SettingsSessions from './SettingsSessions.svelte';
	import SettingsPlaylists from './SettingsPlaylists.svelte';

	interface Props {
		/** The frame's Settings section: the shown section, the sidebar's
		 * sections, and the content. */
		settings: GuiSettingsFrame;
		/** The frame's Status bar facts, which the status line paints. */
		status: GuiStatusBar;
		/** The arrangement the Library-column measurement picks; only Mini
		 * changes the paint (one pane instead of two). */
		arrangement: GuiArrangement;
	}
	const { settings, status, arrangement }: Props = $props();

	/** The section's sidebar label, for the status line and the content's
	 * title. The Services section shows as Sources at the user's choice (design
	 * D13, the *Sources label* bullet): only the label changes. */
	const LABELS: Record<GuiSettingsSection, string> = {
		services: 'Sources',
		playback: 'Playback',
		display: 'Display',
		session: 'Session',
		library: 'Library',
		queue: 'Queue',
		mpv: 'mpv',
		feeds: 'Feeds',
		actions: 'Actions',
		keys: 'Keys',
		sessions: 'Sessions',
		playlists: 'Playlists',
	};

	const mini = $derived(arrangement === 'mini');
	const pane = $derived($settingsUi.pane);
	const entries = $derived(currentSettingsEntries(settings.content, $settingsUi));
	const cursorId = $derived(
		pane === 'content' && $settingsUi.keyboard ? cursorEntryId(entries, $settingsUi) : null,
	);

	let contentEl: HTMLElement | undefined = $state();

	// Keep the cursor row in view while the keyboard walks the content.
	// jsdom (the tests) implements no `scrollIntoView`, so the call is
	// guarded.
	$effect(() => {
		void $settingsUi.contentCursor;
		void $settingsUi.pane;
		const row = contentEl?.querySelector('[data-settings-cursor]');
		if (row instanceof HTMLElement && typeof row.scrollIntoView === 'function') {
			row.scrollIntoView({ block: 'nearest' });
		}
	});
</script>

<!-- A pointer press hides the keyboard cursor, like `:focus-visible`. -->
<svelte:window onpointerdown={hideSettingsCursor} />

<div
	class="flex h-full min-h-0 min-w-0 flex-1 flex-col"
	style:background="var(--canvas-library-column)"
	data-settings-screen=""
	data-settings-arrangement={arrangement}
>
	<div class="flex min-h-0 flex-1">
		{#if !mini || pane === 'sidebar'}
			<div
				class="flex h-full min-h-0 flex-col {mini ? 'min-w-0 flex-1' : 'flex-none'}"
				style:width={mini ? undefined : `${SETTINGS_SIDEBAR_WIDTH_PX}px`}
				data-settings-sidebar-pane=""
			>
				<SettingsSidebar {settings} />
			</div>
		{/if}
		{#if !mini || pane === 'content'}
			<div
				bind:this={contentEl}
				class="h-full min-h-0 min-w-0 flex-1 overflow-y-auto"
				style:box-sizing="border-box"
				style:padding="32px 48px"
				style:background="var(--canvas-library-column)"
				data-settings-content=""
			>
				<div class="flex flex-col" style:max-width="1120px" style:gap="24px">
					<h1
						style:margin="0"
						style:font-size="24px"
						style:font-weight="600"
						style:color="var(--canvas-heading)"
						data-settings-content-title=""
					>
						{LABELS[settings.section]}
					</h1>
					{#if settings.content.kind === 'rows'}
						<SettingsRows
							rows={settings.content.rows}
							{entries}
							{cursorId}
							label={LABELS[settings.section]}
						/>
					{:else if settings.content.kind === 'library'}
						<SettingsLibrary content={settings.content} {entries} {cursorId} />
					{:else if settings.content.kind === 'services'}
						<SettingsServices content={settings.content} {entries} {cursorId} />
					{:else if settings.content.kind === 'feeds'}
						<SettingsFeeds content={settings.content} {entries} {cursorId} />
					{:else if settings.content.kind === 'keys'}
						<SettingsKeys content={settings.content} {entries} {cursorId} />
					{:else if settings.content.kind === 'sessions'}
						<SettingsSessions content={settings.content} {entries} {cursorId} />
					{:else if settings.content.kind === 'playlists'}
						<SettingsPlaylists content={settings.content} {entries} {cursorId} />
					{/if}
				</div>
			</div>
		{/if}
	</div>
	<SettingsStatusBar {status} section={LABELS[settings.section]} />
</div>
