<script lang="ts">
	// The Library Hero's frame (add-gui-library-panel design D5, D8, D15): the
	// one layout Music, Shows, Podcasts, Audiobooks, and Feeds paint, as the TUI
	// paints them through one `HeroContentData` and one Workspace. Three bands, top to bottom:
	//
	//   1. the header — the parent's kicker, title, and facts (Music's artist,
	//      Shows' series), with an optional action on its right;
	//   2. the detail block — the artwork box and, beside it, the picked
	//      item's eyebrow, name, facts, action row, and overview (Music's
	//      album, Shows' selected season with the series' overview). A Hero of one
	//      selected row (Podcasts, Audiobooks, Feeds) paints its kicker and title
	//      here, as Shows does for an episode, and keeps its meta rows in the block;
	//   3. the list — the destination's rows (Music's tracklist, Shows'
	//      season pills over the episodes), passed as the children.
	//
	// A destination supplies facts and rows and never restates the layout, so a
	// fix here lands in all of them. The artwork arm is `LibraryHeroArt`'s: the canvas's
	// 300 px box for a square or portrait cover, and the 16:9 still for a landscape
	// one, which sits beside the text when 320 px or more is free and wraps
	// above it when the Hero is narrower.
	//
	// `attr` names the destination's own attributes: the Music Hero's are
	// `data-library-hero-*` (`attr` empty) and the Shows Hero's are
	// `data-library-shows-hero-*`, so a destination's tests keep their hooks.
	import type { Snippet } from 'svelte';
	import type { GuiArtRef } from '$lib/bridge/library-types';
	import { heroAttr } from '$lib/library/hero';
	import type { Segments } from '$lib/library/hero';
	import LibraryHeroArt from './LibraryHeroArt.svelte';
	import TypedText from './TypedText.svelte';

	interface Props {
		/** The destination's attribute prefix: empty for Music, `shows` for Shows. */
		attr: string;
		/** The root's own attribute, the hook the panel's tests take. */
		root: string;
		/** The header band: always painted, so every Hero shares the one look. The
		 * kicker is a label, and the facts line carries each value's data type
		 * (design D14). */
		header: { kicker: Segments; title: string; facts: Segments };
		/** The header band's trailing control, when the destination has one. */
		headerAction?: Snippet;
		/** The detail block's text: each line paints only when it is non-empty. */
		block: {
			label: string;
			eyebrow?: Segments;
			name?: string;
			facts?: Segments;
			/** Meta rows that carry a role, for a Hero whose facts are rows. */
			meta?: { segments: Segments; role: 'meta' | 'duration' | 'progress' }[];
			overview?: string | null;
		};
		/** The artwork box's shape and its fetched art. */
		artShape: 'landscape' | 'square' | 'portrait';
		art: GuiArtRef | null;
		/** The alternative text of the artwork. */
		artTitle: string;
		/** The artwork box's placeholder while no art is carried. */
		artPlaceholder?: string;
		/** The detail block's action row. */
		actions?: Snippet;
		/** The list band. */
		children?: Snippet;
	}
	const {
		attr,
		root,
		header,
		headerAction,
		block,
		artShape,
		art,
		artTitle,
		artPlaceholder,
		actions,
		children,
	}: Props = $props();

	/** Whether the still is landscape: its text wraps below it when too narrow. */
	const band = $derived(artShape === 'landscape');

	/** The destination's own attribute for one part. */
	function own(name: string, value = ''): Record<string, string> {
		return { [heroAttr(attr, name)]: value };
	}

	/** Whether a line has anything to paint. */
	function painted(segments: Segments | undefined): segments is Segments {
		return (segments ?? []).length > 0;
	}
</script>

<div
	class="flex flex-col"
	style:padding="28px 32px"
	style:gap="28px"
	{...{ [root]: '' }}
>
	<!-- The header band. -->
	<div
		class="flex items-end justify-between"
		style:gap="24px"
		style:padding-bottom="20px"
		style:border-bottom="1px solid var(--canvas-border)"
	>
		<div class="flex min-w-0 flex-col" style:gap="6px">
			<span
				class="font-mono"
				style:font-size="12px"
				style:letter-spacing="0.08em"
				{...own('kicker')}><TypedText segments={header.kicker} /></span
			>
			<h1
				style:margin="0"
				style:font-size="32px"
				style:font-weight="600"
				style:letter-spacing="-0.01em"
				style:color="var(--canvas-heading)"
				{...own('title')}>{header.title}</h1
			>
			{#if painted(header.facts)}
				<span
					class="font-mono"
					style:font-size="13px"
					style:color="var(--canvas-muted)"
					{...own('facts')}><TypedText segments={header.facts} /></span
				>
			{/if}
		</div>
		{@render headerAction?.()}
	</div>

	<!-- The detail block: the artwork and the picked item's text. -->
	<section
		class={band ? 'flex flex-wrap items-start' : 'flex items-start'}
		style:gap={band ? '18px 32px' : '32px'}
		aria-label={block.label}
		{...own('block')}
	>
		<LibraryHeroArt
			shape={artShape}
			{art}
			title={artTitle}
			{attr}
			placeholder={artPlaceholder}
		/>
		<div
			class="flex min-w-0 flex-1 flex-col"
			style:gap="18px"
			style:flex-basis={band ? '320px' : undefined}
			style:padding-top={band ? '0' : '8px'}
		>
			{#if painted(block.eyebrow) || (block.name ?? '') !== '' || painted(block.facts) || (block.meta ?? []).length > 0}
				<div class="flex flex-col" style:gap="6px">
					{#if painted(block.eyebrow)}
						<span
							class="font-mono"
							style:font-size="12px"
							style:letter-spacing="0.08em"
							style:color="var(--canvas-muted)"
							><TypedText segments={block.eyebrow} /></span
						>
					{/if}
					{#if (block.name ?? '') !== ''}
						<h2
							style:margin="0"
							style:font-size="24px"
							style:font-weight="600"
							style:color="var(--canvas-heading)"
							{...own('block-name')}>{block.name}</h2
						>
					{/if}
					{#if painted(block.facts)}
						<span
							class="font-mono"
							style:font-size="13px"
							style:color="var(--canvas-muted)"
							{...own('block-facts')}><TypedText segments={block.facts} /></span
						>
					{/if}
					{#each block.meta ?? [] as row, index (index)}
						<span
							class={row.role === 'meta' ? 'font-mono' : 'font-mono tabular-nums'}
							style:font-size="13px"
							style:color={row.role === 'progress' ? 'var(--canvas-accent)' : 'var(--canvas-muted)'}
							{...own('role', row.role)}><TypedText segments={row.segments} /></span
						>
					{/each}
				</div>
			{/if}
			{#if actions !== undefined}
				<div class="relative flex flex-wrap items-center" style:gap="8px">
					{@render actions()}
				</div>
			{/if}
			{#if (block.overview ?? '') !== ''}
				<p
					class="min-w-0"
					style:margin="0"
					style:font-size="14px"
					style:line-height="1.5"
					style:color="var(--canvas-muted)"
					{...own('overview')}>{block.overview}</p
				>
			{/if}
		</div>
	</section>

	{@render children?.()}
</div>
