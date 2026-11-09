// The Library Status bar's location (add-gui-library-panel design D10): the
// destination, the selected collection names, and the picked item's artist or
// title, joined by ` › ` as the canvas prints them. Every part reads the
// carried Library content, so the line composes no state of its own.

import type {
	GuiAudiobooksFrame,
	GuiEmbyFrame,
	GuiHomeFrame,
	GuiLibraryTab,
	GuiMusicFrame,
	GuiShowsFrame,
} from '$lib/bridge/types';

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

/** The Shows location's last segment (design D10, D12): the title of the row
 * the core selected, which is the row the Hero shows. A frame whose selection
 * names no carried row leaves the segment out. */
function showsSelectedTitle(shows: GuiShowsFrame | null): string | null {
	if (shows === null || shows.selected === null) {
		return null;
	}
	return shows.rows.find((row) => row.id === shows.selected)?.title ?? null;
}

/** The plain Emby-library location's middle segment (design D10, D12, row
 * 8.1): the selected feed pill's name in feed-group mode, and nothing in
 * plain mode, whose letter rail replaces the pill row. */
function embySelectedGroup(emby: GuiEmbyFrame): string | null {
	if (!emby.feedGroup) {
		return null;
	}
	return emby.collectionNames[0] ?? null;
}

/** The plain Emby-library location's last segment (design D10, D12, row 8.1):
 * the title of the row the core selected, which is the row the Hero shows. A
 * frame whose selection names no carried row leaves the segment out. */
function embySelectedTitle(emby: GuiEmbyFrame | null): string | null {
	if (emby === null || emby.selected === null) {
		return null;
	}
	return emby.rows.find((row) => row.id === emby.selected)?.title ?? null;
}

/** The Audiobooks location's last segment (design D10, D12, row 9.1): the
 * title of the book the core selected, which is the book the Hero shows. A
 * frame whose selection names no carried book leaves the segment out. */
function audiobooksSelectedTitle(audiobooks: GuiAudiobooksFrame | null): string | null {
	if (audiobooks === null || audiobooks.selected === null) {
		return null;
	}
	return audiobooks.rows.find((row) => row.id === audiobooks.selected)?.title ?? null;
}

/** The location line's segments in canvas order. On Home the line reads
 * `Home`, then the selected row's Hero title (design D10, D12). On Shows it
 * reads the destination tab's label, the active view mode's name, and the
 * selected row's title (row 7.1). On a plain Emby library it reads the
 * destination tab's label, the selected feed pill in feed-group mode, and the
 * selected row's title (row 8.1). On an Audiobooks library it reads the
 * destination tab's label and the selected book's title (row 9.1). A segment
 * the content does not fill drops, so an unpicked Hero leaves the destination
 * and the collections, and a destination outside the tabs leaves the rest. */
export function libraryLocationSegments(
	tabs: readonly GuiLibraryTab[],
	currentTab: number | null,
	music: GuiMusicFrame | null,
	home: GuiHomeFrame | null = null,
	shows: GuiShowsFrame | null = null,
	emby: GuiEmbyFrame | null = null,
	audiobooks: GuiAudiobooksFrame | null = null,
): string[] {
	if (home !== null) {
		return ['Home', home.hero?.title ?? null].filter(
			(segment): segment is string => segment !== null && segment.trim() !== '',
		);
	}
	if (shows !== null) {
		return [destinationLabel(tabs, currentTab), ...shows.collectionNames, showsSelectedTitle(shows)].filter(
			(segment): segment is string => segment !== null && segment.trim() !== '',
		);
	}
	if (emby !== null) {
		return [
			destinationLabel(tabs, currentTab),
			embySelectedGroup(emby),
			embySelectedTitle(emby),
		].filter((segment): segment is string => segment !== null && segment.trim() !== '');
	}
	if (audiobooks !== null) {
		return [
			destinationLabel(tabs, currentTab),
			audiobooksSelectedTitle(audiobooks),
		].filter((segment): segment is string => segment !== null && segment.trim() !== '');
	}
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
	home: GuiHomeFrame | null = null,
	shows: GuiShowsFrame | null = null,
	emby: GuiEmbyFrame | null = null,
	audiobooks: GuiAudiobooksFrame | null = null,
): string {
	return libraryLocationSegments(
		tabs,
		currentTab,
		music,
		home,
		shows,
		emby,
		audiobooks,
	).join(' › ');
}
