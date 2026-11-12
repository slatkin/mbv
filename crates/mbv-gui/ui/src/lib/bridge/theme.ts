// The theme layer (design D7): the theme section's role colours go into CSS
// custom
// properties on `:root`, and `app.css` points the shadcn-svelte variables at
// them, so shadcn components paint in the mbv palette with no second palette.

import type { GuiTheme } from './types';

/**
 * The CSS custom property a role colour writes to (design D7): the `mbv`
 * prefix plus the role's camelCase wire name, such as `--mbv-textPrimary`.
 * The prefix keeps the role properties clear of the shadcn-svelte variables
 * that point at them (`accent` would otherwise collide with `--accent`).
 */
export function roleProperty(role: string): string {
	return `--mbv-${role}`;
}

/**
 * The `var(--mbv-<role>)` value a part paints in: the custom property a role
 * colour reads from, as every component's inline styles name it.
 */
export function roleVar(role: string): string {
	return `var(${roleProperty(role)})`;
}

/**
 * The `var(--canvas-<name>)` value a part paints in: the Library and queue
 * canvas hexes, which name no theme role slot. Every painter reads them from
 * here, so the palette's canvas names live in one place.
 */
export function canvasVar(name: string): string {
	return `var(--canvas-${name})`;
}

/**
 * A transport glyph's colour: its role while actionable, the muted role when
 * not — the availability gate the TUI's glyph colours carry.
 */
export function glyphColor(available: boolean, role: string): string {
	return available ? roleVar(role) : roleVar('textMuted');
}

/**
 * Writes a frame's theme into CSS custom properties on `:root` (design D7).
 * Only the roles the frame carries are set; a role the frame omits keeps its
 * last value, so a theme application is a replace, not a reset. The chrome
 * fills write to the same `--mbv-<name>` properties the shadcn-svelte
 * chrome variables point at (`--mbv-background` and the rest).
 */
export function applyTheme(theme: GuiTheme, root: HTMLElement = document.documentElement): void {
	for (const [role, rgb] of Object.entries(theme.roles)) {
		root.style.setProperty(roleProperty(role), `rgb(${rgb.r}, ${rgb.g}, ${rgb.b})`);
	}
	for (const [name, rgb] of Object.entries(theme.chrome)) {
		root.style.setProperty(roleProperty(name), `rgb(${rgb.r}, ${rgb.g}, ${rgb.b})`);
	}
}
