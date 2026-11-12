// The Library Status bar's location (add-gui-library-panel design D10): the
// destination, the selected collection names, and the picked item's artist or
// title, joined by ` › ` as the canvas prints them. Every part reads the
// carried Library sections, so the line composes no state of its own: the
// active destination's list part and detail part narrow on their `kind`.

import type {
	GuiDestinationList,
	GuiLibraryDetail,
	GuiLibraryTab,
	GuiMusicHero,
} from '$lib/bridge/types';

/** The current destination's label: the tab whose position the frame marks
 * current, absent while the active destination is not one of the tabs. */
function destinationLabel(
	tabs: readonly GuiLibraryTab[],
	currentTab: number | null,
): string | null {
	return tabs.find((tab) => tab.position === currentTab)?.label ?? null;
}

/** The picked item's name (design D5, D10): the Hero's artist, or the album's
 * title while the Service names no artist. Absent while nothing is picked. */
function pickedName(hero: GuiMusicHero | null): string | null {
	if (hero === null) {
		return null;
	}
	return hero.artist.trim() === '' ? hero.album.name : hero.artist;
}

/** The location line's last segment a destination's rows name (design D10,
 * D12): the title of the row the core selected, which is the row the Hero
 * shows. A selection that names no carried row leaves the segment out. */
function selectedTitle(
	selected: string | null,
	rows: readonly { id: string; title: string }[],
): string | null {
	if (selected === null) {
		return null;
	}
	return rows.find((row) => row.id === selected)?.title ?? null;
}

/** The location line's middle segment a pill row names (design D10, D12): the
 * selected pill's own name. A pill row with no selected pill leaves the segment
 * out. */
function selectedPill(
	pills: readonly { selected: boolean; name: string }[],
): string | null {
	return pills.find((pill) => pill.selected)?.name ?? null;
}

/** The plain Emby-library location's middle segment (design D10, D12, row
 * 8.1): the selected feed pill's name in feed-group mode, and nothing in
 * plain mode, whose letter rail replaces the pill row. */
function embySelectedGroup(list: { feedGroup: boolean; collectionNames: string[] }): string | null {
	if (!list.feedGroup) {
		return null;
	}
	return list.collectionNames[0] ?? null;
}

/** The location line's raw segments for the active destination, in canvas
 * order. A `null` segment drops out when the line is joined. */
function segmentsOf(
	tabs: readonly GuiLibraryTab[],
	currentTab: number | null,
	list: GuiDestinationList | null,
	detail: GuiLibraryDetail | null,
): (string | null)[] {
	const label = destinationLabel(tabs, currentTab);
	switch (list?.kind) {
		case 'home':
			return ['Home', detail?.kind === 'home' ? (detail.hero?.title ?? null) : null];
		case 'shows':
			return [
				label,
				...list.collectionNames,
				selectedTitle(detailSelected(detail), list.rows),
			];
		case 'emby':
			return [
				label,
				embySelectedGroup(list),
				selectedTitle(detailSelected(detail), list.rows),
			];
		case 'audiobooks':
			return [label, selectedTitle(detailSelected(detail), list.rows)];
		case 'podcasts':
			return [
				label,
				selectedPill(list.pills),
				selectedTitle(detailSelected(detail), list.rows),
			];
		case 'feeds':
			return [
				label,
				selectedPill(list.pills),
				selectedTitle(detailSelected(detail), list.rows),
			];
		case 'music':
			return [
				label,
				...list.collectionNames,
				pickedName(detail?.kind === 'music' ? detail.hero : null),
			];
		default:
			return [label];
	}
}

/** The selected row's id of a row-list destination's detail part. Music and
 * Home carry no selected row id, so they read `null`. */
function detailSelected(detail: GuiLibraryDetail | null): string | null {
	if (detail === null || detail.kind === 'music' || detail.kind === 'home') {
		return null;
	}
	return detail.selected;
}

/** The location line's segments in canvas order. On Home the line reads
 * `Home`, then the selected row's Hero title (design D10, D12). On Shows it
 * reads the destination tab's label, the active view mode's name, and the
 * selected row's title (row 7.1). On a plain Emby library it reads the
 * destination tab's label, the selected feed pill in feed-group mode, and the
 * selected row's title (row 8.1). On an Audiobooks library it reads the
 * destination tab's label and the selected book's title (row 9.1). On a
 * Podcasts library it reads the destination tab's label, the selected pill,
 * and the selected episode's title (row 10.1). On a Feeds destination it
 * reads the destination tab's label, the selected pill, and the selected
 * entry's title (row 11.1). A segment the content does not fill drops, so an
 * unpicked Hero leaves the destination and the collections, and a destination
 * outside the tabs leaves the rest. */
export function libraryLocationSegments(
	tabs: readonly GuiLibraryTab[],
	currentTab: number | null,
	list: GuiDestinationList | null,
	detail: GuiLibraryDetail | null,
): string[] {
	return segmentsOf(tabs, currentTab, list, detail).filter(
		(segment): segment is string => segment !== null && segment.trim() !== '',
	);
}

/** The location line the Library Status bar paints: its segments joined by the
 * canvas's separator. */
export function libraryLocation(
	tabs: readonly GuiLibraryTab[],
	currentTab: number | null,
	list: GuiDestinationList | null,
	detail: GuiLibraryDetail | null,
): string {
	return libraryLocationSegments(tabs, currentTab, list, detail).join(' › ');
}
