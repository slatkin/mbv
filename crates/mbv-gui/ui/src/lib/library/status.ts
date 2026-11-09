// The Library Status bar's location (add-gui-library-panel design D10): the
// destination, the selected collection names, and the picked item's artist or
// title, joined by ` › ` as the canvas prints them. Every part reads the
// carried Library content, so the line composes no state of its own.

import type { GuiLibraryTab, GuiMusicFrame } from '$lib/bridge/types';

/** The current destination's label: the tab whose position the frame marks
 * current, absent while the active destination is not one of the tabs. */
function destinationLabel(
	tabs: readonly GuiLibraryTab[],
	currentTab: number | null,
): string | null {
	return tabs.find((tab) => tab.position === currentTab)?.label ?? null;
}

/** The selected collection name (design D6): the name the core derived once
 * and carried on the Music frame, so the line and the list header cannot
 * disagree. One group is always selected, so the line names one collection; a
 * library that holds no group names none. */
function selectedCollections(music: GuiMusicFrame | null): string[] {
	return music?.collectionNames ?? [];
}

/** The picked item's name (design D5, D10): the Hero's artist, or the album's
 * title while the Service names no artist. Absent while nothing is picked. */
function pickedName(music: GuiMusicFrame | null): string | null {
	const hero = music?.hero ?? null;
	if (hero === null) {
		return null;
	}
	return hero.artist.trim() === '' ? hero.album.name : hero.artist;
}

/** The location line's segments in canvas order. A segment the content does
 * not fill drops, so an unpicked Hero leaves the destination and the
 * collections, and a destination outside the tabs leaves the rest. */
export function libraryLocationSegments(
	tabs: readonly GuiLibraryTab[],
	currentTab: number | null,
	music: GuiMusicFrame | null,
): string[] {
	return [
		destinationLabel(tabs, currentTab),
		...selectedCollections(music),
		pickedName(music),
	].filter((segment): segment is string => segment !== null && segment.trim() !== '');
}

/** The location line the Library Status bar paints: its segments joined by the
 * canvas's separator. */
export function libraryLocation(
	tabs: readonly GuiLibraryTab[],
	currentTab: number | null,
	music: GuiMusicFrame | null,
): string {
	return libraryLocationSegments(tabs, currentTab, music).join(' › ');
}
