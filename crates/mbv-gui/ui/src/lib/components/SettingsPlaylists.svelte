<script lang="ts">
	// The Playlists section (add-gui-settings-screen design D9, D13): the saved
	// playlists in a card, and the open playlist's items in one. Each playlist
	// offers the TUI's actions — open, play, shuffle, add, rename, delete,
	// refresh — and each item play, shuffle, and add, all as the canvas's
	// buttons on 36 px rows. The rename field is the window content's draft; a
	// delete raises the core's confirm.
	import type { GuiPlaylistAction, GuiSettingsContent } from '$lib/bridge/types';
	import { entryId } from '$lib/settings/entries';
	import type { SettingsEntry } from '$lib/settings/entries';
	import {
		blurSettingsFieldOnLeave,
		focusSettingsField,
		pressSettingsEntry,
		setRenameDraft,
		settingsUi,
	} from '$lib/settings/state';
	import SettingsButton from './settings/SettingsButton.svelte';
	import SettingsCard from './settings/SettingsCard.svelte';
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

	/** The three whole-playlist and whole-item actions, in paint order. */
	const ACTIONS: readonly { action: GuiPlaylistAction; label: string }[] = [
		{ action: 'play', label: 'Play' },
		{ action: 'shuffle', label: 'Shuffle' },
		{ action: 'enqueue', label: 'Add' },
	];

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

	/** A refresh control's painted text: the load flag while it runs. */
	function refreshLabel(loading: boolean): string {
		return loading ? 'Refreshing…' : 'Refresh';
	}
</script>

{#snippet playlistActions(id: string)}
	<div class="flex flex-wrap" style:gap="6px">
		{#each ACTIONS as control (control.action)}
			{@const actionId = entryId.playlistAction(id, control.action)}
			<SettingsButton
				entry={actionId}
				cursor={actionId === cursorId}
				onpress={() => onpress(actionId)}>{control.label}</SettingsButton
			>
		{/each}
		<SettingsButton
			entry={entryId.playlistRename(id)}
			cursor={entryId.playlistRename(id) === cursorId}
			onpress={() => onpress(entryId.playlistRename(id))}>Rename</SettingsButton
		>
		<SettingsButton
			variant="destructive"
			entry={entryId.playlistDelete(id)}
			cursor={entryId.playlistDelete(id) === cursorId}
			onpress={() => onpress(entryId.playlistDelete(id))}>Delete</SettingsButton
		>
	</div>
{/snippet}

{#snippet renameField()}
	<div
		class="flex"
		style:padding="4px 0"
		data-settings-rename=""
		onfocusin={() => focusSettingsField('rename')}
		onfocusout={(event) => blurSettingsFieldOnLeave(event, 'rename')}
	>
		<input
			bind:this={renameEl}
			type="text"
			aria-label="New playlist name"
			style:flex="1"
			value={$settingsUi.rename?.name ?? ''}
			data-settings-field="rename"
			oninput={(event) => setRenameDraft(event.currentTarget.value)}
		/>
	</div>
{/snippet}

<div
	class="grid"
	style:grid-template-columns="repeat(2, minmax(0, 1fr))"
	style:gap="20px"
	style:align-items="start"
	data-settings-playlists=""
>
	{#if content.open === null}
		<SettingsCard title="Playlists">
			{#snippet header()}
				<SettingsButton
					entry={entryId.playlistRefresh(null)}
					cursor={entryId.playlistRefresh(null) === cursorId}
					onpress={() => onpress(entryId.playlistRefresh(null))}
					>{refreshLabel(content.loading)}</SettingsButton
				>
			{/snippet}
			{#if content.playlists.length === 0}
				<div style:font-size="13px" style:color="var(--canvas-muted)">No saved playlists</div>
			{/if}
			<div class="flex flex-col">
				{#each content.playlists as playlist (playlist.id)}
					{@const rowId = entryId.playlist(playlist.id)}
					<SettingsEntryRow
						id={rowId}
						cursor={rowId === cursorId}
						{onpress}
						label={playlist.name}
						trailing={`${playlist.itemCount} items`}
					/>
					{@render playlistActions(playlist.id)}
					{#if $settingsUi.rename?.id === playlist.id}
						{@render renameField()}
					{/if}
					<div style:height="8px"></div>
				{/each}
			</div>
		</SettingsCard>
	{:else}
		{@const open = content.open}
		<SettingsCard title={open.name}>
			{#snippet header()}
				<div class="flex" style:gap="8px">
					<SettingsButton
						variant="ghost"
						entry={entryId.playlistBack()}
						cursor={entryId.playlistBack() === cursorId}
						onpress={() => onpress(entryId.playlistBack())}>‹ Back</SettingsButton
					>
					<SettingsButton
						entry={entryId.playlistRefresh(open.id)}
						cursor={entryId.playlistRefresh(open.id) === cursorId}
						onpress={() => onpress(entryId.playlistRefresh(open.id))}
						>{refreshLabel(content.openLoading)}</SettingsButton
					>
				</div>
			{/snippet}
			{@render playlistActions(open.id)}
			{#if $settingsUi.rename?.id === open.id}
				{@render renameField()}
			{/if}
			<div class="flex flex-col">
				{#each open.items as item (item.id)}
					<div class="flex items-center" style:height="36px" style:gap="8px">
						<span
							class="min-w-0 flex-1 truncate"
							style:font-size="14px"
							style:color="var(--canvas-text)">{item.name}</span
						>
						{#each ACTIONS as control (control.action)}
							{@const itemActionId = entryId.playlistItemAction(
								open.id,
								item.id,
								control.action,
							)}
							<SettingsButton
								entry={itemActionId}
								cursor={itemActionId === cursorId}
								onpress={() => onpress(itemActionId)}>{control.label}</SettingsButton
							>
						{/each}
					</div>
				{/each}
			</div>
		</SettingsCard>
	{/if}
</div>
