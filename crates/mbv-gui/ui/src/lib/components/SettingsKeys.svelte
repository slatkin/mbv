<script lang="ts">
	// The Keys section (add-gui-settings-screen design D9, D13): the configured
	// prefix chord and one card for each action group, every card holding 36 px
	// rows of buttons. Each action row shows the effective router chords, its
	// prefix-namespace chord where one is assigned, and the changed-from-default
	// mark. Each chord is its own capture target: activating it arms the
	// capture, and the next press is the candidate chord. The frame's last
	// refusal paints under the edited row.
	import type { GuiSettingsContent, GuiSettingsKeyRow } from '$lib/bridge/types';
	import { entryId } from '$lib/settings/entries';
	import type { SettingsEntry } from '$lib/settings/entries';
	import { pressSettingsEntry, settingsUi } from '$lib/settings/state';
	import SettingsButton from './settings/SettingsButton.svelte';
	import SettingsCard from './settings/SettingsCard.svelte';

	interface Props {
		/** The Keys section's content: the prefix chord, the action rows, and
		 * the last refusal. */
		content: Extract<GuiSettingsContent, { kind: 'keys' }>;
		/** The section's full entry list. */
		entries: SettingsEntry[];
		/** The entry id under the content cursor; `null` while the sidebar
		 * holds the keyboard. */
		cursorId: string | null;
	}
	const { content, entries, cursorId }: Props = $props();

	const onpress = (id: string) => pressSettingsEntry(entries, id);

	/** One card's worth of rows: the group's name and its actions in paint
	 * order. */
	interface KeyGroup {
		section: string;
		rows: GuiSettingsKeyRow[];
	}

	/** The rows grouped into one card per section, in the frame's order. */
	const groups = $derived.by<KeyGroup[]>(() => {
		const out: KeyGroup[] = [];
		for (const row of content.rows) {
			const last = out[out.length - 1];
			if (last !== undefined && last.section === row.section) {
				last.rows.push(row);
			} else {
				out.push({ section: row.section, rows: [row] });
			}
		}
		return out;
	});

	/** The capture target's painted text: "press a key" while its capture
	 * waits, the assigned chords otherwise. */
	function targetText(target: string, chords: string): string {
		return $settingsUi.capture?.target === target ? 'press a key…' : chords;
	}

	/** Whether `id` is the entry under the content cursor. */
	function cursor(id: string): boolean {
		return id === cursorId;
	}
</script>

{#snippet chord(id: string, target: string, chords: string)}
	<SettingsButton entry={id} cursor={cursor(id)} onpress={() => onpress(id)}
		><span class="font-mono">{targetText(target, chords)}</span></SettingsButton
	>
{/snippet}

{#snippet refusal(action: string)}
	{#if content.refusal?.action === action}
		<div
			style:padding="0 0 6px"
			style:font-size="13px"
			style:color="var(--canvas-destructive)"
			data-settings-key-refusal="">{content.refusal.text}</div
		>
	{/if}
{/snippet}

<div
	class="grid"
	style:grid-template-columns="repeat(2, minmax(0, 1fr))"
	style:gap="20px"
	style:align-items="start"
	data-settings-keys=""
>
	<SettingsCard title="Prefix chord" rows>
		<div
			class="flex items-center"
			style:height="36px"
			style:gap="8px"
			data-settings-key-row="prefix"
		>
			<span class="min-w-0 flex-1 truncate" style:font-size="14px" style:color="var(--canvas-heading)"
				>Assigned</span
			>
			{@render chord(entryId.keyTarget('prefix'), 'prefix', content.prefix ?? 'none')}
			{#if content.prefix !== null}
				{@const clearId = entryId.keyClear('prefix')}
				<SettingsButton
					variant="destructive"
					entry={clearId}
					label="Clear the prefix chord"
					cursor={cursor(clearId)}
					onpress={() => onpress(clearId)}>✕</SettingsButton
				>
			{/if}
		</div>
		{@render refusal('prefix')}
	</SettingsCard>
	{#each groups as group (group.section)}
		<SettingsCard title={group.section} rows>
			{#each group.rows as row (row.id)}
				{@const targetId = entryId.keyTarget(row.id)}
				{@const prefixTargetId = entryId.keyTarget(`prefix:${row.id}`)}
				{@const resetId = entryId.keyReset(row.id)}
				{@const clearId = entryId.keyClear(`prefix:${row.id}`)}
				<div
					class="flex items-center"
					style:height="36px"
					style:gap="8px"
					data-settings-key-row={row.id}
				>
					<span
						class="min-w-0 flex-1 truncate"
						style:font-size="14px"
						style:color={row.changed ? 'var(--canvas-accent)' : 'var(--canvas-heading)'}
						>{row.label}{row.changed ? ' *' : ''}</span
					>
					{@render chord(targetId, row.id, row.chords.join(' '))}
					{#if row.prefixAddressable}
						{@render chord(prefixTargetId, `prefix:${row.id}`, row.prefixChord ?? '—')}
					{/if}
					{#if row.changed}
						<SettingsButton
							variant="ghost"
							entry={resetId}
							label={`Reset ${row.label} to its default`}
							cursor={cursor(resetId)}
							onpress={() => onpress(resetId)}>↺</SettingsButton
						>
					{/if}
					{#if row.prefixChord !== null}
						<SettingsButton
							variant="destructive"
							entry={clearId}
							label={`Clear ${row.label}'s prefix chord`}
							cursor={cursor(clearId)}
							onpress={() => onpress(clearId)}>✕</SettingsButton
						>
					{/if}
				</div>
				{@render refusal(row.id)}
			{/each}
		</SettingsCard>
	{/each}
</div>
