<script lang="ts">
	// The Settings screen's sidebar (add-gui-settings-screen design D11): the
	// fixed section list in the Queue column's place. It paints the frame's
	// `sections` in order — a section whose every row the GUI hides never
	// arrives — with the cursor the window content owns and the frame's open
	// section marked. A press selects the section, as Enter on the cursor row
	// does; in Mini the sidebar is the pane it replaces.
	import type { GuiSettingsFrame, GuiSettingsSection } from '$lib/bridge/types';
	import { selectSettingsSection, settingsUi } from '$lib/settings/state';

	interface Props {
		/** The open Settings section: the frame's shown `section`, the
		 * `sections` the sidebar lists, and nothing more. */
		settings: GuiSettingsFrame;
	}
	const { settings }: Props = $props();

	/** Each section's sidebar label. */
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

	let listEl: HTMLElement | undefined = $state();

	// A press on a section row selects it: the cursor takes the row, the
	// keyboard takes the sidebar, and the open command the Enter key sends
	// moves the keyboard to the content — in Mini the content is what then
	// shows (the spec's *Narrow window* requirement).
	function press(index: number, section: GuiSettingsSection): void {
		settingsUi.update((ui) => ({ ...ui, pane: 'sidebar', sidebarCursor: index }));
		selectSettingsSection(section);
	}

	// Keep the cursor row in view while the keyboard walks the list. jsdom
	// (the tests) implements no `scrollIntoView`, so the call is guarded.
	$effect(() => {
		void $settingsUi.sidebarCursor;
		const row = listEl?.querySelector('[data-settings-cursor]');
		if (row instanceof HTMLElement && typeof row.scrollIntoView === 'function') {
			row.scrollIntoView({ block: 'nearest' });
		}
	});
</script>

<nav
	class="flex h-full min-h-0 flex-col"
	style:background="var(--canvas-queue-column)"
	aria-label="Settings sections"
	data-settings-sidebar=""
>
	<div
		class="flex flex-none items-center font-mono"
		style:height="40px"
		style:padding="0 16px"
		style:font-size="13px"
		style:color="var(--canvas-muted)"
		style:border-bottom="1px solid var(--canvas-border)"
		data-settings-sidebar-head=""
	>
		Settings
	</div>
	<div bind:this={listEl} class="min-h-0 flex-1 overflow-y-auto">
		{#each settings.sections as section, index (section)}
			{@const current = section === settings.section}
			{@const cursor = index === $settingsUi.sidebarCursor && $settingsUi.pane === 'sidebar'}
			<button
				type="button"
				class="flex w-full items-center text-left"
				style:height="36px"
				style:padding="0 16px"
				style:gap="8px"
				style:font-size="14px"
				style:background={cursor ? 'var(--canvas-selected)' : 'none'}
				style:border="0"
				style:color={current ? 'var(--canvas-heading)' : 'var(--canvas-muted)'}
				aria-current={current ? 'true' : undefined}
				data-settings-section={section}
				data-settings-cursor={cursor ? '' : undefined}
				onclick={() => press(index, section)}
			>
				<span
					class="flex-none"
					style:width="8px"
					style:color="var(--canvas-accent)"
					aria-hidden="true">{current ? '●' : ''}</span
				>
				{LABELS[section]}
			</button>
		{/each}
	</div>
</nav>
