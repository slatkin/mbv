// add-tauri-gui-client task 3.1: every keydown forwards to `key` with the
// DOM fields and its default action prevented (design D6), and pointer and
// wheel events forward to `pointer` as `DomPointer` (design D4). The Tauri
// IPC is mocked; the asserted payloads are the serde wire shapes of
// bridge/keys.rs and bridge/pointer.rs.

import { invoke } from '@tauri-apps/api/core';
import { beforeEach, expect, test, vi } from 'vitest';
import { installInput } from './input';

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
	invokeMock.mockReset();
	invokeMock.mockResolvedValue(undefined);
});

test('a keydown calls key with the DOM fields and prevents the default', () => {
	const detach = installInput(window);
	const event = new KeyboardEvent('keydown', {
		key: 'N',
		code: 'KeyN',
		shiftKey: true,
		altKey: true,
		cancelable: true,
	});
	window.dispatchEvent(event);
	detach();

	expect(event.defaultPrevented).toBe(true);
	expect(invokeMock).toHaveBeenCalledWith('key', {
		event: {
			key: 'N',
			code: 'KeyN',
			metaKey: false,
			repeat: false,
			shiftKey: true,
			ctrlKey: false,
			altKey: true,
		},
	});
});

test('a pointer press calls pointer with its position and button', () => {
	const detach = installInput(window);
	window.dispatchEvent(
		new PointerEvent('pointerdown', {
			button: 0,
			buttons: 1,
			clientX: 10,
			clientY: 20,
			ctrlKey: true,
		}),
	);
	detach();

	expect(invokeMock).toHaveBeenCalledWith('pointer', {
		event: {
			kind: 'press',
			button: 0,
			buttons: 1,
			x: 10,
			y: 20,
			metaKey: false,
			shiftKey: false,
			ctrlKey: true,
			altKey: false,
			deltaX: 0,
			deltaY: 0,
		},
	});
});

test('a wheel event calls pointer with the delta in logical pixels', () => {
	const detach = installInput(window);
	window.dispatchEvent(
		new WheelEvent('wheel', {
			deltaMode: 0,
			deltaX: 0,
			deltaY: -120,
			clientX: 4,
			clientY: 8,
			button: -1,
		}),
	);
	detach();

	expect(invokeMock).toHaveBeenCalledWith(
		'pointer',
		expect.objectContaining({
			event: expect.objectContaining({ kind: 'wheel', deltaX: 0, deltaY: -120 }),
		}),
	);
});

test('a line-mode wheel delta converts to pixels', () => {
	const detach = installInput(window);
	window.dispatchEvent(new WheelEvent('wheel', { deltaMode: 1, deltaX: 0, deltaY: 3 }));
	detach();

	expect(invokeMock).toHaveBeenCalledWith(
		'pointer',
		expect.objectContaining({
			event: expect.objectContaining({ kind: 'wheel', deltaY: 48 }),
		}),
	);
});

test('a pointercancel calls pointer with the neutral cancel', () => {
	const detach = installInput(window);
	window.dispatchEvent(
		new PointerEvent('pointercancel', {
			button: 0,
			buttons: 1,
			clientX: 3,
			clientY: 4,
		}),
	);
	detach();

	expect(invokeMock).toHaveBeenCalledWith(
		'pointer',
		expect.objectContaining({
			event: expect.objectContaining({
				kind: 'cancel',
				button: 0,
				buttons: 1,
				x: 3,
				y: 4,
			}),
		}),
	);
});

test('a pointerleave cancels only while a button is held', () => {
	const detach = installInput(window);
	window.dispatchEvent(new PointerEvent('pointerleave', { buttons: 0 }));
	expect(invokeMock).not.toHaveBeenCalled();

	window.dispatchEvent(new PointerEvent('pointerleave', { buttons: 1 }));
	detach();

	expect(invokeMock).toHaveBeenCalledWith(
		'pointer',
		expect.objectContaining({
			event: expect.objectContaining({ kind: 'cancel', buttons: 1 }),
		}),
	);
});

test('a window blur calls pointer with the neutral cancel', () => {
	const detach = installInput(window);
	window.dispatchEvent(new Event('blur'));
	detach();

	expect(invokeMock).toHaveBeenCalledWith('pointer', {
		event: {
			kind: 'cancel',
			button: -1,
			buttons: 0,
			x: 0,
			y: 0,
			metaKey: false,
			shiftKey: false,
			ctrlKey: false,
			altKey: false,
			deltaX: 0,
			deltaY: 0,
		},
	});
});

test('the cleanup removes the listeners', () => {
	const detach = installInput(window);
	detach();
	window.dispatchEvent(new KeyboardEvent('keydown', { key: 'n', cancelable: true }));

	expect(invokeMock).not.toHaveBeenCalled();
});
