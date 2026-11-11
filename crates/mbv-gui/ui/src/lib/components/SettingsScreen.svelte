<script lang="ts">
	// The Settings screen (add-gui-settings-screen design D11, ADR 0037): the
	// sidebar in the Queue column's place and the shown section's content in
	// the Library column's place, with the status line closing the screen.
	// The sidebar takes the Queue column's expanded width — the dragged or
	// the default width, whatever the collapse toggle, because the 72 px
	// rail cannot hold the list — and paints no divider. Below the
	// `libraryArrangement` Mini threshold (measured as the window width
	// minus that width) only the pane that holds the keyboard shows.
	// The root carries `min-w-0 flex-1`: `App.svelte` gives it a row of a
	// flex container, so without them the screen shrinks to its content
	// width instead of taking the canvas (live finding, Settings screen).
	import type { GuiArrangement } from '$lib/bridge/arrangement';
	import type { GuiSettingsFrame, GuiSettingsSection, GuiStatusBar } from '$lib/bridge/types';
	import { currentSettingsEntries, cursorEntryId, settingsUi } from '$lib/settings/state';
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
		/** The sidebar's column width: the Queue column's expanded width. */
		sidebarWidth: number;
		/** The arrangement the Library-column measurement picks; only Mini
		 * changes the paint (one pane instead of two). */
		arrangement: GuiArrangement;
	}
	const { settings, status, sidebarWidth, arrangement }: Props = $props();

	/** The section's sidebar label, for the status line. */
	const LABELS: Record<GuiSettingsSection, string> = {
		services: 'Services',
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
		pane === 'content' ? cursorEntryId(entries, $settingsUi) : null,
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
				style:width={mini ? undefined : `${sidebarWidth}px`}
				data-settings-sidebar-pane=""
			>
				<SettingsSidebar {settings} />
			</div>
		{/if}
		{#if !mini || pane === 'content'}
			<div
				bind:this={contentEl}
				class="h-full min-h-0 min-w-0 flex-1 overflow-y-auto"
				style:background="var(--canvas-library-panel)"
				data-settings-content=""
			>
				{#if settings.content.kind === 'rows'}
					<SettingsRows rows={settings.content.rows} {entries} {cursorId} />
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
		{/if}
	</div>
	<SettingsStatusBar {status} section={LABELS[settings.section]} />
</div>
