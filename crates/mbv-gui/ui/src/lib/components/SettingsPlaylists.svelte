<script lang="ts">
	// The Playlists section (add-gui-settings-screen design D9): the saved
	// playlists, and the open playlist's items. Each playlist offers the
	// TUI's actions — open, play, shuffle, add, rename, delete, refresh —
	// and each item play, shuffle, and add. The rename field is the window
	// content's draft; a delete raises the core's confirm.
	import type { GuiSettingsContent } from '$lib/bridge/types';
	import { entryId } from '$lib/settings/entries';
	import type { SettingsEntry } from '$lib/settings/entries';
	import {
		blurSettingsField,
		focusSettingsField,
		pressSettingsEntry,
		setRenameDraft,
		settingsUi,
	} from '$lib/settings/state';
	import SettingsEntryRow from './SettingsEntryRow.svelte';

	interface Props {
		/** The Playlists section's content: the list, the open playlist, and
		 * the loading flags. */
		content: Extract<GuiSettingsContent, { kind: 'playlists' }>;
		/** The section's full entry list. */
		entries: SettingsEntry[];
		/** The entry id under the content cursor; `null` while the sidebar
		 * holds the keyboard. */
		cursorId: string | null;
	}
	const { content, entries, cursorId }: Props = $props();

	const onpress = (id: string) => pressSettingsEntry(entries, id);

	const fieldStyle =
		'background:var(--canvas-library-column);border:1px solid var(--canvas-border);border-radius:6px;color:var(--canvas-heading);font-size:14px;height:32px;padding:0 10px;outline:none;';

	let renameEl: HTMLInputElement | undefined = $state();
	let renameWasOpen = false;

	// A rename field opening hands it the keyboard (the spec's *keyboard
	// reachability*).
	$effect(() => {
		const open = $settingsUi.rename !== null;
		if (open && !renameWasOpen) {
			renameEl?.focus();
		}
		renameWasOpen = open;
	});

	function renameFocusOut(event: FocusEvent): void {
		const host = event.currentTarget;
		const next = event.relatedTarget;
		if (host instanceof HTMLElement && (next === null || !host.contains(next as Node))) {
			blurSettingsField('rename');
		}
	}
</script>

<div data-settings-playlists="">
	{#if content.open === null}
		<SettingsEntryRow
			id={entryId.playlistRefresh(null)}
			cursor={entryId.playlistRefresh(null) === cursorId}
			{onpress}
			label={content.loading ? 'Refreshing…' : 'Refresh'}
		/>
		{#each content.playlists as playlist (playlist.id)}
			{@const rowId = entryId.playlist(playlist.id)}
			<SettingsEntryRow
				id={rowId}
				cursor={rowId === cursorId}
				{onpress}
				label={playlist.name}
				trailing={`${playlist.itemCount} items`}
			/>
			<div class="flex flex-wrap" style:padding="0 16px 4px 32px" style:gap="6px">
				{#each [{ action: 'play', label: 'Play' }, { action: 'shuffle', label: 'Shuffle' }, { action: 'enqueue', label: 'Add' }] as control (control.action)}
					{@const id = entryId.playlistAction(playlist.id, control.action as 'play' | 'shuffle' | 'enqueue')}
					<button
						type="button"
						style:font-size="12px"
						style:padding="2px 8px"
						style:border-radius="4px"
						style:border="1px solid var(--canvas-border)"
						style:background={id === cursorId ? 'var(--canvas-selected)' : 'var(--canvas-well)'}
						style:color="var(--canvas-text)"
						data-settings-entry={id}
						data-settings-cursor={id === cursorId ? '' : undefined}
						onclick={() => onpress(id)}>{control.label}</button
					>
				{/each}
				{#each [{ id: entryId.playlistRename(playlist.id), label: 'Rename' }, { id: entryId.playlistDelete(playlist.id), label: 'Delete' }] as control (control.id)}
					<button
						type="button"
						style:font-size="12px"
						style:padding="2px 8px"
						style:border-radius="4px"
						style:border="1px solid var(--canvas-border)"
						style:background={control.id === cursorId ? 'var(--canvas-selected)' : 'var(--canvas-well)'}
						style:color={control.label === 'Delete'
							? 'var(--canvas-destructive)'
							: 'var(--canvas-text)'}
						data-settings-entry={control.id}
						data-settings-cursor={control.id === cursorId ? '' : undefined}
						onclick={() => onpress(control.id)}>{control.label}</button
					>
				{/each}
			</div>
			{#if $settingsUi.rename?.id === playlist.id}
				<div
					class="flex"
					style:padding="0 16px 8px 32px"
					data-settings-rename=""
					onfocusin={() => focusSettingsField('rename')}
					onfocusout={renameFocusOut}
				>
					<input
						bind:this={renameEl}
						type="text"
						aria-label="New playlist name"
						style={fieldStyle}
						style:flex="1"
						value={$settingsUi.rename.name}
						data-settings-field="rename"
						oninput={(event) => setRenameDraft(event.currentTarget.value)}
					/>
				</div>
			{/if}
		{/each}
	{:else}
		{@const open = content.open}
		<SettingsEntryRow
			id={entryId.playlistBack()}
			cursor={entryId.playlistBack() === cursorId}
			{onpress}
			label="‹ Back"
		/>
		<SettingsEntryRow
			id={entryId.playlistRefresh(open.id)}
			cursor={entryId.playlistRefresh(open.id) === cursorId}
			{onpress}
			label={content.openLoading ? 'Refreshing…' : 'Refresh'}
		/>
		<div
			class="flex items-center"
			style:height="36px"
			style:padding="0 16px"
			style:font-size="14px"
			style:color="var(--canvas-heading)"
			style:border-top="1px solid var(--canvas-border)"
			data-settings-playlist-head="">{open.name}</div
		>
		<div class="flex flex-wrap" style:padding="0 16px 8px" style:gap="6px">
			{#each [{ action: 'play', label: 'Play' }, { action: 'shuffle', label: 'Shuffle' }, { action: 'enqueue', label: 'Add' }] as control (control.action)}
				{@const id = entryId.playlistAction(open.id, control.action as 'play' | 'shuffle' | 'enqueue')}
				<button
					type="button"
					style:font-size="12px"
					style:padding="2px 8px"
					style:border-radius="4px"
					style:border="1px solid var(--canvas-border)"
					style:background={id === cursorId ? 'var(--canvas-selected)' : 'var(--canvas-well)'}
					style:color="var(--canvas-text)"
					data-settings-entry={id}
					data-settings-cursor={id === cursorId ? '' : undefined}
					onclick={() => onpress(id)}>{control.label}</button
				>
			{/each}
			{#each [{ id: entryId.playlistRename(open.id), label: 'Rename' }, { id: entryId.playlistDelete(open.id), label: 'Delete' }] as control (control.id)}
				<button
					type="button"
					style:font-size="12px"
					style:padding="2px 8px"
					style:border-radius="4px"
					style:border="1px solid var(--canvas-border)"
					style:background={control.id === cursorId ? 'var(--canvas-selected)' : 'var(--canvas-well)'}
					style:color={control.label === 'Delete' ? 'var(--canvas-destructive)' : 'var(--canvas-text)'}
					data-settings-entry={control.id}
					data-settings-cursor={control.id === cursorId ? '' : undefined}
					onclick={() => onpress(control.id)}>{control.label}</button
				>
			{/each}
		</div>
		{#if $settingsUi.rename?.id === open.id}
			<div
				class="flex"
				style:padding="0 16px 8px"
				data-settings-rename=""
				onfocusin={() => focusSettingsField('rename')}
				onfocusout={renameFocusOut}
			>
				<input
					bind:this={renameEl}
					type="text"
					aria-label="New playlist name"
					style={fieldStyle}
					style:flex="1"
					value={$settingsUi.rename.name}
					data-settings-field="rename"
					oninput={(event) => setRenameDraft(event.currentTarget.value)}
				/>
			</div>
		{/if}
		{#each open.items as item (item.id)}
			<div class="flex items-center" style:min-height="32px" style:padding="0 16px" style:gap="8px">
				<span class="min-w-0 flex-1 truncate" style:font-size="14px" style:color="var(--canvas-text)">{item.name}</span>
				{#each [{ action: 'play', label: 'Play' }, { action: 'shuffle', label: 'Shuffle' }, { action: 'enqueue', label: 'Add' }] as control (control.action)}
					{@const id = entryId.playlistItemAction(open.id, item.id, control.action as 'play' | 'shuffle' | 'enqueue')}
					<button
						type="button"
						class="flex-none"
						style:font-size="12px"
						style:padding="2px 8px"
						style:border-radius="4px"
						style:border="1px solid var(--canvas-border)"
						style:background={id === cursorId ? 'var(--canvas-selected)' : 'var(--canvas-well)'}
						style:color="var(--canvas-text)"
						data-settings-entry={id}
						data-settings-cursor={id === cursorId ? '' : undefined}
						onclick={() => onpress(id)}>{control.label}</button
					>
				{/each}
			</div>
		{/each}
	{/if}
</div>
