<script lang="ts">
	// The flat Library list's item row (add-gui-library-panel design D15,
	// invariant 30): the title over its subtitle, the date or duration at the
	// right, the cursor and selection fills, and the resume run under the row.
	// Every flat destination list — Emby, Audiobooks, Podcasts, Feeds — paints
	// its rows here, so a fix to one row look lands in every list.
	//
	// A destination supplies its own hook prefix (`attr`), the row's text, and
	// the press, double-click, and context handlers; it never restates the
	// look. Every part paints `data-library-<attr>-<part>`.
	interface Props {
		/** The destination's hook prefix, as in `podcasts`, `feeds`, `emby`, `abs`. */
		attr: string;
		/** The row's own kind value, as in `episode`, `entry`, `book`. */
		kind: string;
		id: string;
		height: number;
		title: string;
		subtitle: string | null;
		/** The trailing text and its hook part: the date gutter or the duration. */
		trail: string | null;
		trailPart: 'date' | 'duration';
		/** The title's colour: the text tone, or the muted tone on a played row. */
		titleColor: string;
		/** The row's painted fill, when the cursor or the selection marks it. */
		background: string | undefined;
		cursor: boolean;
		selected: boolean;
		/** The resume percent of a partly played row, or `null`. */
		progress: number | null;
		onpress: () => void;
		onplay: () => void;
		oncontext?: (event: MouseEvent) => void;
	}
	const {
		attr,
		kind,
		id,
		height,
		title,
		subtitle,
		trail,
		trailPart,
		titleColor,
		background,
		cursor,
		selected,
		progress,
		onpress,
		onplay,
		oncontext,
	}: Props = $props();

	/** The destination's own attribute for one part of the row. */
	function own(part: string, value = ''): Record<string, string> {
		return { [`data-library-${attr}-${part}`]: value };
	}
</script>

<button
	type="button"
	class="relative flex w-full items-center overflow-hidden"
	style:height="{height}px"
	style:padding="0 12px"
	style:gap="10px"
	style:border="0"
	style:border-radius="4px"
	style:text-align="left"
	style:font-size="14px"
	style:background
	style:cursor="pointer"
	{...own('row', kind)}
	{...own('row-id', id)}
	{...cursor ? own('cursor') : {}}
	{...selected ? own('selected') : {}}
	onclick={onpress}
	ondblclick={onplay}
	oncontextmenu={oncontext}
>
	<span class="flex min-w-0 flex-1 flex-col" style:gap="2px">
		<span
			class="truncate"
			style:color={titleColor}
			style:font-weight={selected ? 600 : 400}
			{...own('title')}>{title}</span
		>
		{#if subtitle !== null}
			<span
				class="truncate"
				style:font-size="12px"
				style:color="var(--canvas-muted)"
				{...own('subtitle')}>{subtitle}</span
			>
		{/if}
	</span>
	{#if trail !== null && trail !== ''}
		<span
			class="flex-none font-mono tabular-nums"
			style:font-size="12px"
			style:color="var(--canvas-muted)"
			{...own(trailPart)}>{trail}</span
		>
	{/if}
	{#if progress !== null}
		<!-- The resume position: a thin accent run under the row's bottom edge, at
		the frame's bounded percent. -->
		<span
			class="absolute"
			style:left="0"
			style:bottom="0"
			style:height="2px"
			style:width="{progress}%"
			style:background="var(--canvas-accent)"
			{...own('progress')}
		></span>
	{/if}
</button>
