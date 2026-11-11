<script lang="ts">
	// The Feeds section (add-gui-settings-screen design D9): the feed
	// subscriptions, the add control, and the add/edit form. A subscription's
	// only stable identity is its URL — the edit and the remove address it —
	// so the form's edit mode keeps the URL read-only and sends it back as
	// the identity. While a fetch runs the frame's `adding` flag shows the
	// add in progress.
	import type { GuiSettingsContent } from '$lib/bridge/types';
	import { entryId } from '$lib/settings/entries';
	import type { SettingsEntry } from '$lib/settings/entries';
	import {
		blurSettingsField,
		focusSettingsField,
		pressSettingsEntry,
		setFeedForm,
		settingsUi,
		submitSettingsForm,
	} from '$lib/settings/state';
	import SettingsEntryRow from './SettingsEntryRow.svelte';

	interface Props {
		/** The Feeds section's content: the subscriptions and the pending-add
		 * flag. */
		content: Extract<GuiSettingsContent, { kind: 'feeds' }>;
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

	let formEl: HTMLElement | undefined = $state();
	let formWasOpen = false;

	// A form opening hands its first field the keyboard (the spec's
	// *keyboard reachability*).
	$effect(() => {
		const open = $settingsUi.feedForm !== null;
		if (open && !formWasOpen) {
			formEl?.querySelector('input')?.focus();
		}
		formWasOpen = open;
	});

	function fieldFocusOut(event: FocusEvent): void {
		const host = event.currentTarget;
		const next = event.relatedTarget;
		if (host instanceof HTMLElement && (next === null || !host.contains(next as Node))) {
			blurSettingsField('feed');
		}
	}
</script>

<div data-settings-feeds="">
	<SettingsEntryRow
		id={entryId.feedAdd()}
		cursor={entryId.feedAdd() === cursorId}
		{onpress}
		label="+ Add feed"
	/>
	{#if $settingsUi.feedForm !== null}
		{@const form = $settingsUi.feedForm}
		<div
			bind:this={formEl}
			class="flex flex-col"
			style:padding="8px 16px"
			style:gap="8px"
			style:border-top="1px solid var(--canvas-border)"
			style:border-bottom="1px solid var(--canvas-border)"
			data-settings-feed-form=""
			onfocusin={() => focusSettingsField('feed')}
			onfocusout={fieldFocusOut}
		>
			{#if form.identity !== null}
				<div
					class="truncate"
					style:font-size="13px"
					style:color="var(--canvas-muted)"
					data-settings-feed-identity="">{form.identity}</div
				>
			{/if}
			<input
				type="text"
				placeholder="Name"
				aria-label="Feed name"
				style={fieldStyle}
				value={form.name}
				data-settings-field="name"
				oninput={(event) => setFeedForm({ name: event.currentTarget.value })}
			/>
			{#if form.identity === null}
				<input
					type="text"
					placeholder="Feed URL"
					aria-label="Feed URL"
					style={fieldStyle}
					value={form.url}
					data-settings-field="url"
					oninput={(event) => setFeedForm({ url: event.currentTarget.value })}
				/>
			{/if}
			<select
				aria-label="Feed kind"
				style={fieldStyle}
				value={form.kind}
				data-settings-field="kind"
				onchange={(event) =>
					setFeedForm({ kind: event.currentTarget.value === 'video' ? 'video' : 'audio' })}
			>
				<option value="audio">Audio</option>
				<option value="video">Video</option>
			</select>
			<div class="flex" style:gap="8px">
				<button
					type="button"
					disabled={content.adding}
					style:font-size="13px"
					style:padding="4px 10px"
					style:border-radius="4px"
					style:border="0"
					style:background="var(--canvas-well)"
					style:color={content.adding ? 'var(--canvas-ordinal)' : 'var(--canvas-accent)'}
					style:opacity={content.adding ? '0.7' : undefined}
					data-settings-feed-submit=""
					onclick={() => submitSettingsForm(content)}
					>{content.adding ? 'Adding…' : form.identity === null ? 'Add' : 'Save'}</button
				>
				<button
					type="button"
					style:font-size="13px"
					style:padding="4px 10px"
					style:border-radius="4px"
					style:border="0"
					style:background="var(--canvas-well)"
					style:color="var(--canvas-destructive)"
					data-settings-feed-cancel=""
					onclick={() =>
						settingsUi.update((ui) => ({ ...ui, feedForm: null, textField: null }))}
					>Cancel</button
				>
			</div>
		</div>
	{:else if content.adding}
		<div
			class="flex items-center"
			style:height="36px"
			style:padding="0 16px"
			style:font-size="13px"
			style:color="var(--canvas-muted)"
			data-settings-feed-adding="">Adding…</div
		>
	{/if}
	{#each content.subscriptions as feed (feed.url)}
		{@const feedRowId = entryId.feed(feed.url)}
		{@const removeId = entryId.feedRemove(feed.url)}
		<SettingsEntryRow
			id={feedRowId}
			cursor={feedRowId === cursorId}
			{onpress}
			label={feed.name}
			trailing={feed.kind}
		/>
		<SettingsEntryRow
			id={removeId}
			cursor={removeId === cursorId}
			{onpress}
			label={`Remove ${feed.name}`}
			indent={true}
		/>
	{/each}
</div>
