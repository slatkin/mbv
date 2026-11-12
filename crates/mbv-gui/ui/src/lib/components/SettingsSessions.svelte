<script lang="ts">
	// The Sessions section (add-gui-settings-screen design D9, D13): the same
	// playback targets the header's chooser lists, each marked while it is the
	// Owner's attached output, in a card of 36 px rows. Selecting one sends
	// `selectTarget` — the chooser's own path — and the screen stays open.
	import type { GuiSettingsContent } from '$lib/bridge/types';
	import { entryId } from '$lib/settings/entries';
	import type { SettingsEntry } from '$lib/settings/entries';
	import { pressSettingsEntry } from '$lib/settings/state';
	import SettingsCard from './settings/SettingsCard.svelte';
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

<div
	class="grid"
	style:grid-template-columns="repeat(2, minmax(0, 1fr))"
	style:gap="20px"
	style:align-items="start"
	data-settings-sessions=""
>
	<SettingsCard title="Playback targets" rows>
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
	</SettingsCard>
</div>
