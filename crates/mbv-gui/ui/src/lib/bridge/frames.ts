// The frame store (design D4): subscribes to the `mbv://frame` event and
// keeps only the last frame. The store holds no interaction state — the core
// thread owns that — so a window reload re-subscribes and paints the next
// frame it receives with the core's state intact.
//
// Components read the frame through Svelte's store contract (`$frames`);
// non-component callers read the `frame` getter.

import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { get, writable } from 'svelte/store';
import type { Readable } from 'svelte/store';
import { applyTheme } from './theme';
import type { GuiTheme, GuiWindowFrame } from './types';

/** The event the core thread emits its frames through; `FRAME_EVENT` in main.rs. */
export const FRAME_EVENT = 'mbv://frame';

/** The theme a frame carries, or `null` when it carries none. */
function frameTheme(frame: GuiWindowFrame): GuiTheme | null {
	return 'slice' in frame ? frame.slice.theme : frame.attach.theme;
}

/**
 * The last frame the core thread emitted. Each event replaces the frame
 * whole and applies its theme when it carries one (design D7).
 */
export class FrameStore implements Readable<GuiWindowFrame | null> {
	readonly #store = writable<GuiWindowFrame | null>(null);

	readonly subscribe: Readable<GuiWindowFrame | null>['subscribe'] = (run) =>
		this.#store.subscribe(run);

	/** The last frame received, or `null` before the first one arrives. */
	get frame(): GuiWindowFrame | null {
		return get(this.#store);
	}

	/**
	 * Subscribes to `mbv://frame` and resolves to the unlisten function;
	 * the caller runs it on unmount.
	 */
	listen(): Promise<UnlistenFn> {
		return listen<GuiWindowFrame>(FRAME_EVENT, (event) => {
			this.#store.set(event.payload);
			const theme = frameTheme(event.payload);
			if (theme) {
				applyTheme(theme);
			}
		});
	}
}

/** The window's frame store. */
export const frames = new FrameStore();
