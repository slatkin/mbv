<script lang="ts">
	// The F2-row sections (add-gui-settings-screen design D9, D13): Playback,
	// Display, Session, Queue, Mpv, and Actions — each row's label, its value,
	// and the way it is edited, painted with the shared Settings parts.
	//
	// A toggle row is a switch, a pick row is a pick button with its value
	// menu, a cycle row is a button carrying the value it steps, an action
	// row is a button, and an info row is its value text with no control. A choice list is a checkbox-grid card whose Apply runs
	// the row's own Apply entry, so the grid and the keyboard share one effect.
	// The cards sit in the canvas's two-column grid, which leaves a
	// single-card section (mpv) inside the canvas's 560 px.
	//
	// The Library section reuses this for its choice rows, passing the
	// section's full entry list so the cursor index is the screen's own.
	import type { GuiSettingRow } from '$lib/bridge/types';
	import { entryId } from '$lib/settings/entries';
	import type { SettingsEntry } from '$lib/settings/entries';
	import { markedChoices, pressSettingsEntry, settingsUi } from '$lib/settings/state';
	import SettingsButton from './settings/SettingsButton.svelte';
	import SettingsCard from './settings/SettingsCard.svelte';
	import SettingsCheckboxGrid from './settings/SettingsCheckboxGrid.svelte';
	import SettingsPickButton from './settings/SettingsPickButton.svelte';
	import SettingsRow from './settings/SettingsRow.svelte';
	import SettingsSwitch from './settings/SettingsSwitch.svelte';

	interface Props {
		/** The section's F2 rows, in paint order. */
		rows: GuiSettingRow[];
		/** The whole section's entry list (the Library section's includes its
		 * routes): a press resolves the row's index and activation from it. */
		entries: SettingsEntry[];
		/** The entry id the content cursor rests on while the content pane
		 * holds the keyboard; `null` while the sidebar does. */
		cursorId: string | null;
		/** The card's accessible name: the section's name. */
		label?: string;
	}
	const { rows, entries, cursorId, label = undefined }: Props = $props();

	const onpress = (id: string) => pressSettingsEntry(entries, id);

	/** The rows one card holds as setting rows: a toggle, a cycle, a pick, or an
	 * info row. */
	const settingRows = $derived(
		rows.filter(
			(row) =>
				row.kind === 'toggle' ||
				row.kind === 'cycle' ||
				row.kind === 'pick' ||
				row.kind === 'info',
		),
	);
	/** The action rows, which the canvas paints as buttons. */
	const actionRows = $derived(rows.filter((row) => row.kind === 'action'));
	/** The choice-list rows, one checkbox-grid card each. */
	const choiceRows = $derived(rows.filter((row) => row.kind === 'choiceList'));

</script>

<div
	class="grid"
	style:grid-template-columns="repeat(2, minmax(0, 1fr))"
	style:gap="20px"
	style:align-items="start"
	data-settings-rows=""
>
	{#if settingRows.length > 0}
		<SettingsCard rows {label}>
			{#each settingRows as row, index (row.key)}
				{@const id = entryId.setting(row.key)}
				<SettingsRow
					label={row.label}
					divider={index < settingRows.length - 1}
					cursor={id === cursorId}
				>
					{#if row.kind === 'toggle'}
						<SettingsSwitch
							label={row.label}
							checked={row.flag ?? false}
							onpress={() => onpress(id)}
						/>
					{:else if row.kind === 'info'}
						<span data-settings-info={row.key} style:color="var(--canvas-dim)">{row.value}</span>
					{:else if row.kind === 'pick'}
						<SettingsPickButton
							setting={row.key}
							label={row.label}
							value={row.value}
							onpress={() => onpress(id)}
						/>
					{:else}
						<SettingsButton onpress={() => onpress(id)}>{row.value}</SettingsButton>
					{/if}
				</SettingsRow>
			{/each}
		</SettingsCard>
	{/if}
	{#if actionRows.length > 0}
		<SettingsCard {label}>
			<div class="flex flex-wrap" style:gap="8px">
				{#each actionRows as row (row.key)}
					<SettingsButton onpress={() => onpress(entryId.setting(row.key))}
						>{row.label}</SettingsButton
					>
				{/each}
			</div>
		</SettingsCard>
	{/if}
	{#each choiceRows as row (row.key)}
		<SettingsCheckboxGrid
			title={row.label}
			rowKey={row.key}
			options={(row.choices ?? []).map((choice) => ({
				id: choice.id,
				label: choice.label,
			}))}
			selected={markedChoices(row, $settingsUi)}
			{cursorId}
			{onpress}
		/>
	{/each}
</div>
