<script lang="ts">
	// The F2-row sections (add-gui-settings-screen design D9): Playback,
	// Display, Session, Queue, Mpv, and Actions — each row's label, its value
	// text, and the way it is edited. A toggle, cycle, or action row is one
	// activatable row; a choice list paints its header, its marks, and its
	// Apply control, so the list edits in place as the spec requires.
	//
	// The Library section reuses this for its choice rows, passing the
	// section's full entry list so the cursor index is the screen's own.
	import type { GuiSettingRow } from '$lib/bridge/types';
	import { entryId } from '$lib/settings/entries';
	import type { SettingsEntry } from '$lib/settings/entries';
	import { markedChoices, pressSettingsEntry, settingsUi } from '$lib/settings/state';
	import SettingsEntryRow from './SettingsEntryRow.svelte';

	interface Props {
		/** The section's F2 rows, in paint order. */
		rows: GuiSettingRow[];
		/** The whole section's entry list (the Library section's includes its
		 * routes): a press resolves the row's index and activation from it. */
		entries: SettingsEntry[];
		/** The entry id the content cursor rests on while the content pane
		 * holds the keyboard; `null` while the sidebar does. */
		cursorId: string | null;
	}
	const { rows, entries, cursorId }: Props = $props();

	const onpress = (id: string) => pressSettingsEntry(entries, id);
</script>

<div data-settings-rows="">
	{#each rows as row (row.key)}
		{#if row.kind === 'choiceList'}
			<!-- The choice list's header: the row itself activates nothing in
				the GUI — its marks and its Apply control edit it in place. -->
			<div
				class="flex items-center"
				style:height="36px"
				style:padding="0 16px"
				style:font-size="14px"
				style:color="var(--canvas-heading)"
				data-settings-row={row.key}
			>
				<span class="min-w-0 flex-1 truncate">{row.label}</span>
				<span class="flex-none truncate" style:color="var(--canvas-muted)">{row.value}</span>
			</div>
			{#each row.choices ?? [] as choice (choice.id)}
				{@const id = entryId.choice(row.key, choice.id)}
				<SettingsEntryRow
					{id}
					cursor={id === cursorId}
					{onpress}
					label={choice.label}
					marked={markedChoices(row, $settingsUi).includes(choice.id)}
					indent={true}
				/>
			{/each}
			<SettingsEntryRow
				id={entryId.choicesApply(row.key)}
				cursor={entryId.choicesApply(row.key) === cursorId}
				{onpress}
				label="Apply"
				indent={true}
			/>
		{:else}
			<SettingsEntryRow
				id={entryId.setting(row.key)}
				cursor={entryId.setting(row.key) === cursorId}
				{onpress}
				label={row.label}
				trailing={row.value}
			/>
		{/if}
	{/each}
</div>
