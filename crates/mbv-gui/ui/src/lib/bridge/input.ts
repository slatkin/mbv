// The input layer (designs D4 and D6): forwards DOM events to the core
// thread's commands and interprets nothing. Every keydown goes to `key` with
// its default action prevented, and pointer press, release, move, and wheel
// events go to `pointer`. The Rust side owns the key translation and the
// gesture policy; an event the neutral vocabulary cannot represent drops
// there, as the TUI's terminal edge drops it.

import { invoke } from '@tauri-apps/api/core';
import type {
	DomKey,
	DomPointer,
	DomPointerKind,
	LayoutReport,
} from './types';

/**
 * The DOM `KeyboardEvent` fields the `key` command carries (design D6). The
 * field names are the serde wire names of `DomKey` in bridge/keys.rs.
 */
export function domKey(event: KeyboardEvent): DomKey {
	return {
		key: event.key,
		code: event.code,
		metaKey: event.metaKey,
		repeat: event.repeat,
		shiftKey: event.shiftKey,
		ctrlKey: event.ctrlKey,
		altKey: event.altKey,
	};
}

/** The modifier flags and position every pointer kind carries. */
function pointerFields(
	kind: DomPointerKind,
	event: PointerEvent | WheelEvent,
): DomPointer {
	return {
		kind,
		button: event.button,
		buttons: event.buttons,
		x: event.clientX,
		y: event.clientY,
		metaKey: event.metaKey,
		shiftKey: event.shiftKey,
		ctrlKey: event.ctrlKey,
		altKey: event.altKey,
		deltaX: 0,
		deltaY: 0,
	};
}

/** A `pointerdown`, `pointerup`, or `pointermove` as a `DomPointer`. */
export function domPointer(
	kind: Extract<DomPointerKind, 'press' | 'release' | 'move'>,
	event: PointerEvent,
): DomPointer {
	return pointerFields(kind, event);
}

/**
 * The pixels one DOM `deltaMode` line stands for. The core reads only the
 * delta's sign (bridge/pointer.rs: one wheel event is one uniform step), so
 * this conversion exists only to honour the DTO's logical-pixel unit, not to
 * size a scroll.
 */
const PIXELS_PER_LINE = 16;

/**
 * A `wheel` event as a `DomPointer`, with the DOM delta converted to logical
 * pixels (design D4: `DomPointer` carries logical pixels; the front end
 * converts lines and pages before it sends). The conversion is unit-only:
 * the core decides what a wheel step means, so no notch or gesture policy
 * lives here.
 */
export function domWheel(event: WheelEvent): DomPointer {
	const pointer = pointerFields('wheel', event);
	// deltaMode: 0 pixels, 1 lines, 2 pages (DOM_DELTA_*). A page stands for
	// the window's height.
	const unit =
		event.deltaMode === 1
			? PIXELS_PER_LINE
			: event.deltaMode === 2
				? window.innerHeight
				: 1;
	pointer.deltaX = event.deltaX * unit;
	pointer.deltaY = event.deltaY * unit;
	return pointer;
}

/**
 * A cancelled gesture as a `DomPointer`: a `pointercancel`, a `pointerleave`
 * with a button held, or the window's blur. The webview lost the gesture, so
 * its release never arrives; the core drops the drag state and sends no
 * move. A blur carries no pointer fields, so the position and button fall to
 * neutral placeholders.
 */
export function domCancel(event?: PointerEvent): DomPointer {
	return {
		kind: 'cancel',
		button: event?.button ?? -1,
		buttons: event?.buttons ?? 0,
		x: event?.clientX ?? 0,
		y: event?.clientY ?? 0,
		metaKey: event?.metaKey ?? false,
		shiftKey: event?.shiftKey ?? false,
		ctrlKey: event?.ctrlKey ?? false,
		altKey: event?.altKey ?? false,
		deltaX: 0,
		deltaY: 0,
	};
}

/** Sends a `DomKey` through the `key` command (design D4). */
export function sendKey(key: DomKey): Promise<unknown> {
	return invoke('key', { event: key });
}

/** Sends a `DomPointer` through the `pointer` command (design D4). */
export function sendPointer(pointer: DomPointer): Promise<unknown> {
	return invoke('pointer', { event: pointer });
}

/**
 * Sends a `LayoutReport` through the `report_layout` command (design D4).
 * The surface units call this after each paint that moved a region, and on
 * mount so the core emits the first frame without waiting for a change.
 */
export function sendLayoutReport(report: LayoutReport): Promise<unknown> {
	return invoke('report_layout', { report });
}

/**
 * Installs the window's input forwarding: every keydown goes to `key` with
 * its default action prevented (design D6 — no webview default such as
 * reload, find, zoom, or back navigation runs), and pointer press, release,
 * move, and wheel events go to `pointer` (design D4). A `pointercancel`, a
 * `pointerleave` with a button held, and the window's blur forward the
 * neutral cancel, so a gesture the webview loses never leaves a drag armed
 * (row 6.5). Returns the cleanup that removes every listener.
 */
export function installInput(target: Window = window): () => void {
	const onKeydown = (event: KeyboardEvent) => {
		event.preventDefault();
		void sendKey(domKey(event));
	};
	const onPointerDown = (event: PointerEvent) => {
		void sendPointer(domPointer('press', event));
	};
	const onPointerUp = (event: PointerEvent) => {
		void sendPointer(domPointer('release', event));
	};
	const onPointerMove = (event: PointerEvent) => {
		void sendPointer(domPointer('move', event));
	};
	const onWheel = (event: WheelEvent) => {
		void sendPointer(domWheel(event));
	};
	const onPointerCancel = (event: PointerEvent) => {
		void sendPointer(domCancel(event));
	};
	const onPointerLeave = (event: PointerEvent) => {
		// A leave with no button held owes no release; only a gesture in
		// flight needs the cancel.
		if (event.buttons !== 0) {
			void sendPointer(domCancel(event));
		}
	};
	const onBlur = () => {
		void sendPointer(domCancel());
	};

	target.addEventListener('keydown', onKeydown);
	target.addEventListener('pointerdown', onPointerDown);
	target.addEventListener('pointerup', onPointerUp);
	target.addEventListener('pointermove', onPointerMove);
	target.addEventListener('wheel', onWheel);
	target.addEventListener('pointercancel', onPointerCancel);
	target.addEventListener('pointerleave', onPointerLeave);
	target.addEventListener('blur', onBlur);

	return () => {
		target.removeEventListener('keydown', onKeydown);
		target.removeEventListener('pointerdown', onPointerDown);
		target.removeEventListener('pointerup', onPointerUp);
		target.removeEventListener('pointermove', onPointerMove);
		target.removeEventListener('wheel', onWheel);
		target.removeEventListener('pointercancel', onPointerCancel);
		target.removeEventListener('pointerleave', onPointerLeave);
		target.removeEventListener('blur', onBlur);
	};
}
