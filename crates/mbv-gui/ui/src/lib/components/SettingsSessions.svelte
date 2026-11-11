<script lang="ts">
	// The Sessions section (add-gui-settings-screen design D9): the same
	// playback targets the header's chooser lists, each marked while it is
	// the Owner's attached output. Selecting one sends `selectTarget` — the
	// chooser's own path — and the screen stays open.
	import type { GuiSettingsContent } from '$lib/bridge/types';
	import { entryId } from '$lib/settings/entries';
	import type { SettingsEntry } from '$lib/settings/entries';
	import { pressSettingsEntry } from '$lib/settings/state';
	import SettingsEntryRow from './SettingsEntryRow.svelte';

	interface Props {
		/** The Sessions section's content: the playback targets. */
		content: Extract<GuiSettingsContent, { kind: 'sessions' }>;
		/** The section's full entry list. */
		entries: SettingsEntry[];
		/** The entry id under the content cursor; `null` while the sidebar
		 * holds the keyboard. */
		cursorId: string | null;
	}
	const { content, entries, cursorId }: Props = $props();

	const onpress = (id: string) => pressSettingsEntry(entries, id);
</script>

<div data-settings-sessions="">
	{#each content.targets as target, index (index)}
		{@const id = entryId.target(index)}
		<SettingsEntryRow
			{id}
			cursor={id === cursorId}
			{onpress}
			label={target.label}
			trailing={target.isActive ? '● Active' : null}
		/>
	{/each}
</div>
