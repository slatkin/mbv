// The typed readers of the Library sections (gui-change-driven-sections design
// D8): the `libraryList` and `libraryDetail` stores, narrowed on the
// destination's `kind`. A reader returns the part only while the active
// destination is that one, and `null` otherwise. The core builds both parts in
// one pass, so a list part and its detail part are `null` together.

import { get } from 'svelte/store';
import { libraryDetail, libraryList } from '$lib/bridge/sections';
import type {
	GuiAudiobooksDetail,
	GuiAudiobooksList,
	GuiDestinationList,
	GuiEmbyDetail,
	GuiEmbyList,
	GuiFeedsDetail,
	GuiFeedsList,
	GuiHomeDetail,
	GuiHomeList,
	GuiLibraryDetail,
	GuiMusicDetail,
	GuiMusicList,
	GuiPodcastsDetail,
	GuiPodcastsList,
	GuiShowsDetail,
	GuiShowsList,
} from '$lib/bridge/types';

/** The active destination's kind, or `null` while no presented destination is
 * active. */
export function activeLibraryDestination(): GuiDestinationList['kind'] | null {
	return get(libraryList)?.destination.kind ?? null;
}

/** The Music list part, or `null` while another destination is active. */
export function libraryMusicList(): GuiMusicList | null {
	const destination = get(libraryList)?.destination;
	return destination?.kind === 'music' ? destination : null;
}

/** The Music detail part, or `null` while another destination is active. */
export function libraryMusicDetail(): GuiMusicDetail | null {
	const detail = get(libraryDetail);
	return detail?.kind === 'music' ? detail : null;
}

/** The Home list part, or `null` while another destination is active. */
export function libraryHomeList(): GuiHomeList | null {
	const destination = get(libraryList)?.destination;
	return destination?.kind === 'home' ? destination : null;
}

/** The Home detail part, or `null` while another destination is active. */
export function libraryHomeDetail(): GuiHomeDetail | null {
	const detail = get(libraryDetail);
	return detail?.kind === 'home' ? detail : null;
}

/** The Shows list part, or `null` while another destination is active. */
export function libraryShowsList(): GuiShowsList | null {
	const destination = get(libraryList)?.destination;
	return destination?.kind === 'shows' ? destination : null;
}

/** The Shows detail part, or `null` while another destination is active. */
export function libraryShowsDetail(): GuiShowsDetail | null {
	const detail = get(libraryDetail);
	return detail?.kind === 'shows' ? detail : null;
}

/** The plain Emby-library list part, or `null` while another destination is
 * active. */
export function libraryEmbyList(): GuiEmbyList | null {
	const destination = get(libraryList)?.destination;
	return destination?.kind === 'emby' ? destination : null;
}

/** The plain Emby-library detail part, or `null` while another destination is
 * active. */
export function libraryEmbyDetail(): GuiEmbyDetail | null {
	const detail = get(libraryDetail);
	return detail?.kind === 'emby' ? detail : null;
}

/** The Audiobooks list part, or `null` while another destination is active. */
export function libraryAudiobooksList(): GuiAudiobooksList | null {
	const destination = get(libraryList)?.destination;
	return destination?.kind === 'audiobooks' ? destination : null;
}

/** The Audiobooks detail part, or `null` while another destination is active. */
export function libraryAudiobooksDetail(): GuiAudiobooksDetail | null {
	const detail = get(libraryDetail);
	return detail?.kind === 'audiobooks' ? detail : null;
}

/** The Podcasts list part, or `null` while another destination is active. */
export function libraryPodcastsList(): GuiPodcastsList | null {
	const destination = get(libraryList)?.destination;
	return destination?.kind === 'podcasts' ? destination : null;
}

/** The Podcasts detail part, or `null` while another destination is active. */
export function libraryPodcastsDetail(): GuiPodcastsDetail | null {
	const detail = get(libraryDetail);
	return detail?.kind === 'podcasts' ? detail : null;
}

/** The Feeds list part, or `null` while another destination is active. */
export function libraryFeedsList(): GuiFeedsList | null {
	const destination = get(libraryList)?.destination;
	return destination?.kind === 'feeds' ? destination : null;
}

/** The Feeds detail part, or `null` while another destination is active. */
export function libraryFeedsDetail(): GuiFeedsDetail | null {
	const detail = get(libraryDetail);
	return detail?.kind === 'feeds' ? detail : null;
}

/** One destination's list part and detail part, narrowed together on the
 * destination's `kind`: the destination's painters take both. */
export type LibraryPair =
	| { kind: 'music'; list: GuiMusicList; detail: GuiMusicDetail }
	| { kind: 'home'; list: GuiHomeList; detail: GuiHomeDetail }
	| { kind: 'shows'; list: GuiShowsList; detail: GuiShowsDetail }
	| { kind: 'emby'; list: GuiEmbyList; detail: GuiEmbyDetail }
	| { kind: 'audiobooks'; list: GuiAudiobooksList; detail: GuiAudiobooksDetail }
	| { kind: 'podcasts'; list: GuiPodcastsList; detail: GuiPodcastsDetail }
	| { kind: 'feeds'; list: GuiFeedsList; detail: GuiFeedsDetail };

/** The pair of the two sections' parts, or `null` while either is absent or
 * the two name different destinations. */
export function libraryPair(
	list: GuiDestinationList | null,
	detail: GuiLibraryDetail | null,
): LibraryPair | null {
	if (list === null || detail === null) {
		return null;
	}
	if (list.kind === 'music' && detail.kind === 'music') {
		return { kind: 'music', list, detail };
	}
	if (list.kind === 'home' && detail.kind === 'home') {
		return { kind: 'home', list, detail };
	}
	if (list.kind === 'shows' && detail.kind === 'shows') {
		return { kind: 'shows', list, detail };
	}
	if (list.kind === 'emby' && detail.kind === 'emby') {
		return { kind: 'emby', list, detail };
	}
	if (list.kind === 'audiobooks' && detail.kind === 'audiobooks') {
		return { kind: 'audiobooks', list, detail };
	}
	if (list.kind === 'podcasts' && detail.kind === 'podcasts') {
		return { kind: 'podcasts', list, detail };
	}
	if (list.kind === 'feeds' && detail.kind === 'feeds') {
		return { kind: 'feeds', list, detail };
	}
	return null;
}
