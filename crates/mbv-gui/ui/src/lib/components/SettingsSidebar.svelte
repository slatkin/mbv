<script lang="ts">
	// The Settings screen's sidebar (add-gui-settings-screen design D11, D13's
	// canvas `nav`): the fixed section list in the Queue column's place. It
	// paints the frame's `sections` in order — a section whose every row the
	// GUI hides never arrives — each with its Lucide icon and its name, the
	// cursor the window content owns, and the open section's row in the accent
	// fill with ink text. A press selects the section, as Enter on the cursor
	// row does; in Mini the sidebar is the pane it replaces. The Back control
	// above the title closes the screen (design D11 *Mouse-centric*), so a user
	// can leave without a key, as Esc from the sidebar does.
	import {
		ArrowLeft,
		Cast,
		Clock,
		Keyboard,
		Library,
		ListOrdered,
		ListPlus,
		Monitor,
		Play,
		Rss,
		Server,
		SquarePlay,
		Zap,
	} from '@lucide/svelte';
	import type { Component } from 'svelte';
	import type { GuiSettingsFrame, GuiSettingsSection } from '$lib/bridge/types';
	import { sendSettingsCommand } from '$lib/bridge/settings';
	import { selectSettingsSection, settingsUi } from '$lib/settings/state';

	interface Props {
		/** The open Settings section: the frame's shown `section`, the
		 * `sections` the sidebar lists, and nothing more. */
		settings: GuiSettingsFrame;
	}
	const { settings }: Props = $props();

	/** Each section's sidebar label. The Services section shows as Sources at
	 * the user's choice (design D13, the *Sources label* bullet): only the label
	 * changes, so the section id and every command keep their name. */
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

	/** Each section's Lucide icon (design D13): the closest icon to the
	 * artboard's custom path, because the GUI's rule stays Lucide only. */
	const ICONS: Record<GuiSettingsSection, Component<{ size?: number }>> = {
		services: Server,
		playback: Play,
		display: Monitor,
		session: Clock,
		library: Library,
		queue: ListOrdered,
		mpv: SquarePlay,
		feeds: Rss,
		actions: Zap,
		keys: Keyboard,
		sessions: Cast,
		playlists: ListPlus,
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

	// Back closes the screen, as Esc from the sidebar does (the spec's *A Back
	// control in the sidebar closes the screen*).
	function back(): void {
		void sendSettingsCommand({ command: 'close' });
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
	style:border-right="1px solid var(--canvas-border)"
	style:padding="20px 12px"
	style:gap="2px"
	aria-label="Settings sections"
	data-settings-sidebar=""
>
	<button
		type="button"
		class="flex flex-none items-center text-left"
		style:height="32px"
		style:padding="0 12px"
		style:gap="8px"
		style:border="0"
		style:border-radius="8px"
		style:background="none"
		style:color="var(--canvas-muted)"
		style:font-size="14px"
		data-settings-back=""
		onclick={back}
	>
		<span class="flex flex-none items-center" aria-hidden="true"><ArrowLeft size={16} /></span>
		Back
	</button>
	<div
		class="flex flex-none items-baseline"
		style:padding="0 12px 14px"
		style:font-size="20px"
		style:font-weight="600"
		style:color="var(--canvas-heading)"
		data-settings-sidebar-title=""
	>
		Settings
	</div>
	<div bind:this={listEl} class="min-h-0 flex-1 overflow-y-auto">
		{#each settings.sections as section, index (section)}
			{@const current = section === settings.section}
			{@const cursor = index === $settingsUi.sidebarCursor &&
				$settingsUi.pane === 'sidebar' &&
				$settingsUi.keyboard}
			{@const Icon = ICONS[section]}
			<button
				type="button"
				class="flex w-full items-center text-left"
				style:height="38px"
				style:padding="0 12px"
				style:gap="12px"
				style:border="0"
				style:border-radius="8px"
				style:font-size="14px"
				style:background={current
					? 'var(--canvas-accent)'
					: cursor
						? 'var(--canvas-selected)'
						: 'none'}
				style:color={current ? 'var(--canvas-ink)' : 'var(--canvas-text)'}
				style:font-weight={current ? '600' : '400'}
				aria-current={current ? 'page' : undefined}
				data-settings-section={section}
				data-settings-cursor={cursor ? '' : undefined}
				onclick={() => press(index, section)}
			>
				<span
					class="flex flex-none items-center"
					style:color={current ? 'var(--canvas-ink)' : 'var(--canvas-muted)'}
					aria-hidden="true"
				>
					<Icon size={16} />
				</span>
				{LABELS[section]}
			</button>
		{/each}
	</div>
</nav>
