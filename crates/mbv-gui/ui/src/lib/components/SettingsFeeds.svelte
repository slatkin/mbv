<script lang="ts">
	// The Feeds section (add-gui-settings-screen design D9, D13): the feed
	// subscriptions in a card of their own, the add control above them, and the
	// add/edit form as a card beside the list — a recessed well of labelled
	// mono fields with Cancel and the primary submit. A subscription's only
	// stable identity is its URL — the edit and the remove address it — so the
	// form's edit mode keeps the URL read-only and sends it back as the
	// identity. While a fetch runs the frame's `adding` flag shows the add in
	// progress.
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
	import SettingsButton from './settings/SettingsButton.svelte';
	import SettingsCard from './settings/SettingsCard.svelte';
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

	/** The count line the card's header paints. */
	const countLabel = $derived(
		`${content.subscriptions.length} subscription${content.subscriptions.length === 1 ? '' : 's'}`,
	);
</script>

<div
	class="grid"
	style:grid-template-columns="repeat(2, minmax(0, 1fr))"
	style:gap="20px"
	style:align-items="start"
	data-settings-feeds=""
>
	<SettingsCard title="Subscriptions">
		{#snippet header()}
			<span style:font-size="12px" style:color="var(--canvas-partial)">{countLabel}</span>
		{/snippet}
		<div class="flex">
			<SettingsButton
				entry={entryId.feedAdd()}
				cursor={entryId.feedAdd() === cursorId}
				onpress={() => onpress(entryId.feedAdd())}>Add feed</SettingsButton
			>
		</div>
		{#if content.adding}
			<div
				style:font-size="13px"
				style:color="var(--canvas-muted)"
				data-settings-feed-adding="">Adding…</div
			>
		{/if}
		<div class="flex flex-col">
			{#each content.subscriptions as feed (feed.url)}
				{@const feedRowId = entryId.feed(feed.url)}
				{@const removeId = entryId.feedRemove(feed.url)}
				<div class="flex items-center" style:gap="8px">
					<SettingsEntryRow
						id={feedRowId}
						cursor={feedRowId === cursorId}
						{onpress}
						label={feed.name}
						trailing={feed.kind}
					/>
					<SettingsButton
						variant="destructive"
						entry={removeId}
						cursor={removeId === cursorId}
						onpress={() => onpress(removeId)}>Remove</SettingsButton
					>
				</div>
			{/each}
		</div>
	</SettingsCard>
	{#if $settingsUi.feedForm !== null}
		{@const form = $settingsUi.feedForm}
		<SettingsCard title={form.identity === null ? 'Add feed' : 'Edit feed'}>
			<div
				bind:this={formEl}
				class="flex flex-col"
				style:padding="16px"
				style:gap="12px"
				style:background="var(--canvas-well)"
				style:border-radius="8px"
				data-settings-feed-form=""
				onfocusin={() => focusSettingsField('feed')}
				onfocusout={fieldFocusOut}
			>
				{#if form.identity !== null}
					<div
						class="truncate font-mono"
						style:font-size="12px"
						style:color="var(--canvas-muted)"
						data-settings-feed-identity="">{form.identity}</div
					>
				{/if}
				<label
					class="flex flex-col"
					style:gap="6px"
					style:font-size="12px"
					style:color="var(--canvas-muted)"
				>
					Name
					<input
						type="text"
						placeholder="Name"
						aria-label="Feed name"
						value={form.name}
						data-settings-field="name"
						oninput={(event) => setFeedForm({ name: event.currentTarget.value })}
					/>
				</label>
				{#if form.identity === null}
					<label
						class="flex flex-col"
						style:gap="6px"
						style:font-size="12px"
						style:color="var(--canvas-muted)"
					>
						Feed URL
						<input
							type="text"
							placeholder="https://example.com/feed.xml"
							aria-label="Feed URL"
							value={form.url}
							data-settings-field="url"
							oninput={(event) => setFeedForm({ url: event.currentTarget.value })}
						/>
					</label>
				{/if}
				<label
					class="flex flex-col"
					style:gap="6px"
					style:font-size="12px"
					style:color="var(--canvas-muted)"
				>
					Kind
					<select
						aria-label="Feed kind"
						value={form.kind}
						data-settings-field="kind"
						onchange={(event) =>
							setFeedForm({
								kind: event.currentTarget.value === 'video' ? 'video' : 'audio',
							})}
					>
						<option value="audio">Audio</option>
						<option value="video">Video</option>
					</select>
				</label>
				<div class="flex justify-end" style:gap="8px">
					<SettingsButton
						variant="ghost"
						onpress={() =>
							settingsUi.update((ui) => ({ ...ui, feedForm: null, textField: null }))}
						>Cancel</SettingsButton
					>
					<SettingsButton
						variant="primary"
						disabled={content.adding}
						onpress={() => submitSettingsForm(content)}
						>{content.adding
							? 'Adding…'
							: form.identity === null
								? 'Add'
								: 'Save'}</SettingsButton
					>
				</div>
			</div>
		</SettingsCard>
	{/if}
</div>
