// The theme layer (design D7): a frame's role colours go into CSS custom
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
 * Writes a frame's theme into CSS custom properties on `:root` (design D7).
 * Only the roles the frame carries are set; a role the frame omits keeps its
 * last value, so a theme application is a replace, not a reset.
 */
export function applyTheme(theme: GuiTheme, root: HTMLElement = document.documentElement): void {
	for (const [role, rgb] of Object.entries(theme.roles)) {
		root.style.setProperty(roleProperty(role), `rgb(${rgb.r}, ${rgb.g}, ${rgb.b})`);
	}
}
