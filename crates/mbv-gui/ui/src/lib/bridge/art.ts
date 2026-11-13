// The artwork reference every section DTO carries (design D7). It mirrors
// `GuiArtRef` in crates/mbv-gui/src/art.rs field for field. `src` is the
// `mbv-art` URL the window loads; a `loading` reference carries no URL, because
// a fetch is in flight or awaits its retry. The window paints a reference
// through `ArtImage.svelte`, the one art painter.

export type GuiArtRef =
	| { kind: 'image'; src: string }
	| { kind: 'placeholder'; src: string }
	| { kind: 'loading' };
