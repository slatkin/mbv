<script lang="ts">
	// The Hero's list band (add-gui-library-panel design D15, invariant 30): the
	// one table every destination's Workspace paints — Music's tracklist, Shows'
	// episodes, Audiobooks' chapters, and Feeds' entries. The header row, the
	// row grid, the cursor fill, the playing glyph, the duration cell, the
	// resume run, the hover controls, and the loading line are painted here
	// once, so a fix lands in every list.
	//
	// A destination supplies its rows, its cursor, its hover actions, and its
	// hook prefix; it never restates the layout. The hover is the table's own
	// paint: the pointer-over row swaps its number for the play control and
	// shows its actions. A press moves the destination's cursor, and a
	// double-click on a row, like the play control, plays it.
	import type { Component } from 'svelte';
	import { Play } from '@lucide/svelte';

	/** One row of the table, as the destination maps its rows. */
	export interface WorkspaceRow {
		/** The row's stable identity, the value the destination's commands name. */
		id: string;
		/** The row's number cell. */
		number: string;
		name: string;
		duration: string;
		/** Whether the row is the one playing at the playhead. */
		playing?: boolean;
		/** The row's played state, when the destination carries one. */
		state?: 'played' | 'active' | 'ordinary';
		/** The resume position percent, when the row is partly played. */
		progress?: number | null;
	}

	/** One hover action of a row. */
	export interface WorkspaceAction {
		/** The action's hook value and its accessible name's source. */
		key: string;
		label: string;
		icon: Component<{ size?: number }>;
		run: (id: string) => void;
	}

	interface Props {
		/** The section's own attribute: the hook the destination's tests take. */
		root: string;
		/** The section's accessible name. */
		label: string;
		/** The destination's row hook, as in `track`, `shows-episode`, `abs-chapter`:
		 * every part paints `data-library-<hook>-<part>`. */
		hook: string;
		/** The word that names a row's press target: `Track`, `Episode`, `Chapter`. */
		noun: string;
		rows: WorkspaceRow[];
		/** The cursor's row id, or `null`. */
		cursor: string | null;
		actions: WorkspaceAction[];
		/** A press on a row: the destination moves its cursor. */
		onpress: (id: string) => void;
		/** Play a row: the number cell's play control and the row's double-click. */
		onplay: (id: string) => void;
		/** A right-click on a row, when the destination has a row menu. */
		oncontext?: (event: MouseEvent, id: string) => void;
		/** The loading line, while the rows have not landed. */
		loading?: { text: string; attr: string } | null;
	}
	const {
		root,
		label,
		hook,
		noun,
		rows,
		cursor,
		actions,
		onpress,
		onplay,
		oncontext,
		loading = null,
	}: Props = $props();

	/** The row the pointer is over: the table's own paint, never shared state. */
	let hovered = $state<string | null>(null);

	/** A row's title colour: the playing row's heading tone, the muted tone once
	 * played, else the text tone. */
	function nameColor(row: WorkspaceRow): string {
		if (row.playing === true) {
			return 'var(--canvas-heading)';
		}
		return row.state === 'played' ? 'var(--canvas-muted)' : 'var(--canvas-text)';
	}

	/** The destination's own attribute for one part of a row. */
	function own(part: string, value = ''): Record<string, string> {
		return { [`data-library-${hook}${part === '' ? '' : `-${part}`}`]: value };
	}
</script>

<section class="flex flex-col" aria-label={label} {...{ [root]: '' }}>
	{#if loading !== null}
		<span
			class="font-mono"
			style:font-size="13px"
			style:color="var(--canvas-muted)"
			{...{ [loading.attr]: '' }}>{loading.text}</span
		>
	{:else}
		<div
			class="grid items-center font-mono"
			style:grid-template-columns="40px minmax(0, 1fr) 88px 64px"
			style:height="32px"
			style:padding="0 8px"
			style:border-bottom="1px solid var(--canvas-border)"
			style:font-size="12px"
			style:letter-spacing="0.08em"
			style:color="var(--canvas-muted)"
		>
			<span>#</span>
			<span>TITLE</span>
			<span></span>
			<span style:text-align="right">TIME</span>
		</div>
		<ol class="flex flex-col" style:margin="4px 0 0" style:padding="0" style:list-style="none">
			{#each rows as row (row.id)}
				<li
					class="relative grid items-center"
					style:grid-template-columns="40px minmax(0, 1fr) 88px 64px"
					style:height="34px"
					style:padding="0 8px"
					style:border-radius="4px"
					style:background={row.id === cursor ? 'var(--canvas-selected)' : undefined}
					{...own('', row.id)}
					{...row.id === cursor ? own('cursor') : {}}
					{...row.playing === true ? own('playing') : {}}
					{...row.state === undefined ? {} : own('state', row.state)}
					onpointerenter={() => (hovered = row.id)}
					onpointerleave={() => (hovered = null)}
					oncontextmenu={oncontext === undefined ? undefined : (event) => oncontext(event, row.id)}
				>
					<span
						class="flex-none font-mono tabular-nums"
						style:font-size="11px"
						style:color="var(--canvas-ordinal)"
					>
						<span style:display={hovered === row.id ? 'none' : undefined} {...own('number')}
							>{row.number}</span
						>
						<button
							type="button"
							class="cursor-pointer items-center hover:drop-shadow-[0_0_4px_var(--canvas-accent)]"
							style:display={hovered === row.id ? 'flex' : 'none'}
							aria-label="Play {row.name}"
							{...own('action', 'play')}
							onclick={() => onplay(row.id)}
						>
							<Play size={12} fill="var(--canvas-accent)" color="var(--canvas-accent)" />
						</button>
					</span>
					<button
						type="button"
						class="flex min-w-0 items-center"
						style:background="none"
						style:border="0"
						style:padding="0"
						style:text-align="left"
						style:cursor="pointer"
						aria-label="{noun} {row.name}"
						{...own('name')}
						onclick={() => onpress(row.id)}
						ondblclick={() => onplay(row.id)}
					>
						<span class="flex min-w-0 items-center" style:gap="6px">
							{#if row.playing === true}
								<!-- The playing row's play glyph sits beside its title: a highlight
								marks focus only. -->
								<span {...own('glyph')}>
									<Play size={12} fill="var(--canvas-accent)" color="var(--canvas-accent)" />
								</span>
							{/if}
							<span
								class="min-w-0 truncate"
								style:font-size="13.5px"
								style:color={nameColor(row)}>{row.name}</span
							>
						</span>
					</button>
					<span class="flex justify-end" style:gap="2px">
						<span
							class="justify-end"
							style:gap="2px"
							style:display={hovered === row.id ? 'flex' : 'none'}
							{...own('actions')}
						>
							{#each actions as action (action.key)}
								<button
									type="button"
									class="flex items-center justify-center"
									style:width="28px"
									style:height="28px"
									style:background="none"
									style:border="0"
									style:border-radius="4px"
									style:color="var(--canvas-text)"
									style:cursor="pointer"
									aria-label={action.label}
									{...own('action', action.key)}
									onclick={() => action.run(row.id)}
								>
									<action.icon size={14} />
								</button>
							{/each}
						</span>
					</span>
					<span
						class="font-mono tabular-nums"
						style:font-size="12px"
						style:color="var(--canvas-muted)"
						style:text-align="right"
						{...own('duration')}>{row.duration}</span
					>
					{#if row.progress !== null && row.progress !== undefined}
						<!-- The resume position: a thin accent run under the row's bottom
						edge, at the frame's bounded percent. -->
						<span
							class="absolute"
							style:left="0"
							style:bottom="0"
							style:height="2px"
							style:width="{row.progress}%"
							style:background="var(--canvas-accent)"
							{...own('progress')}
						></span>
					{/if}
				</li>
			{/each}
		</ol>
	{/if}
</section>
