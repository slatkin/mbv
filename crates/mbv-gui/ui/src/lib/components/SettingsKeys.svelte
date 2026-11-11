<script lang="ts">
	// The Keys section (add-gui-settings-screen design D9): the configured
	// prefix chord, then one row per `AllClients` action grouped by the shared
	// section vocabulary — each row's effective router chords, its
	// prefix-namespace chord where one is assigned, and the changed-from-default
	// mark. Each chord is its own capture target: activating it arms the
	// capture, and the next press is the candidate chord. The frame's last
	// refusal paints on the edited row.
	import type { GuiSettingsContent } from '$lib/bridge/types';
	import { entryId } from '$lib/settings/entries';
	import type { SettingsEntry } from '$lib/settings/entries';
	import { pressSettingsEntry, settingsUi } from '$lib/settings/state';

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

	/** The small control style of a row's binding buttons. */
	const keyButton =
		'font-size:12px;padding:2px 8px;border-radius:4px;border:1px solid var(--canvas-border);background:var(--canvas-well);color:var(--canvas-text);font-family:monospace;';

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

<div data-settings-keys="">
	<!-- The prefix chord: the `prefix` capture target, with its clear control
		while one is assigned. -->
	<div
		class="flex items-center"
		style:min-height="36px"
		style:padding="4px 16px"
		style:gap="8px"
		style:font-size="14px"
		style:border-bottom="1px solid var(--canvas-border)"
		data-settings-key-row="prefix"
	>
		<span
			class="min-w-0 flex-1 truncate"
			style:color="var(--canvas-heading)"
		>Prefix chord</span
		>
		<button
			type="button"
			style={keyButton}
			style:border-color={cursor(entryId.keyTarget('prefix'))
				? 'var(--canvas-accent)'
				: undefined}
			data-settings-entry={entryId.keyTarget('prefix')}
			data-settings-cursor={cursor(entryId.keyTarget('prefix')) ? '' : undefined}
			onclick={() => onpress(entryId.keyTarget('prefix'))}
			>{targetText('prefix', content.prefix ?? 'none')}</button
		>
		{#if content.prefix !== null}
			<button
				type="button"
				style={keyButton}
				style:color="var(--canvas-destructive)"
				data-settings-entry={entryId.keyClear('prefix')}
				data-settings-cursor={cursor(entryId.keyClear('prefix')) ? '' : undefined}
				onclick={() => onpress(entryId.keyClear('prefix'))}
				aria-label="Clear the prefix chord">✕</button
			>
		{/if}
	</div>
	{#if content.refusal?.action === 'prefix'}
		<div
			style:padding="0 16px 4px"
			style:font-size="13px"
			style:color="var(--canvas-destructive)"
			data-settings-key-refusal="">{content.refusal.text}</div
		>
	{/if}
	{#each content.rows as row, index (row.id)}
		{#if index === 0 || content.rows[index - 1].section !== row.section}
			<div
				class="flex items-center font-mono"
				style:height="32px"
				style:padding="0 16px"
				style:font-size="13px"
				style:color="var(--canvas-muted)"
				style:border-bottom="1px solid var(--canvas-border)"
				data-settings-key-group="">{row.section}</div
			>
		{/if}
		{@const targetId = entryId.keyTarget(row.id)}
		{@const prefixTargetId = entryId.keyTarget(`prefix:${row.id}`)}
		{@const resetId = entryId.keyReset(row.id)}
		{@const clearId = entryId.keyClear(`prefix:${row.id}`)}
		<div
			class="flex items-center"
			style:min-height="36px"
			style:padding="4px 16px"
			style:gap="8px"
			style:font-size="14px"
			data-settings-key-row={row.id}
		>
			<span
				class="min-w-0 flex-1 truncate"
				style:color={row.changed ? 'var(--canvas-accent)' : 'var(--canvas-heading)'}
				>{row.label}{row.changed ? ' *' : ''}</span
			>
			<span class="flex-none" style:color="var(--canvas-muted)" style:font-size="12px"
				>{row.section}</span
			>
			<button
				type="button"
				style={keyButton}
				style:border-color={cursor(targetId) ? 'var(--canvas-accent)' : undefined}
				data-settings-entry={targetId}
				data-settings-cursor={cursor(targetId) ? '' : undefined}
				onclick={() => onpress(targetId)}
				>{targetText(row.id, row.chords.join(' '))}</button
			>
			{#if row.prefixAddressable}
				<button
					type="button"
					style={keyButton}
					style:border-color={cursor(prefixTargetId) ? 'var(--canvas-accent)' : undefined}
					data-settings-entry={prefixTargetId}
					data-settings-cursor={cursor(prefixTargetId) ? '' : undefined}
					onclick={() => onpress(prefixTargetId)}
					>{targetText(`prefix:${row.id}`, row.prefixChord ?? '—')}</button
				>
			{/if}
			{#if row.changed}
				<button
					type="button"
					style={keyButton}
					data-settings-entry={resetId}
					data-settings-cursor={cursor(resetId) ? '' : undefined}
					onclick={() => onpress(resetId)}
					aria-label="Reset {row.label} to its default">↺</button
				>
			{/if}
			{#if row.prefixChord !== null}
				<button
					type="button"
					style={keyButton}
					style:color="var(--canvas-destructive)"
					data-settings-entry={clearId}
					data-settings-cursor={cursor(clearId) ? '' : undefined}
					onclick={() => onpress(clearId)}
					aria-label="Clear {row.label}'s prefix chord">✕</button
				>
			{/if}
		</div>
		{#if content.refusal?.action === row.id}
			<div
				style:padding="0 16px 4px"
				style:font-size="13px"
				style:color="var(--canvas-destructive)"
				data-settings-key-refusal="">{content.refusal.text}</div
			>
		{/if}
	{/each}
</div>
