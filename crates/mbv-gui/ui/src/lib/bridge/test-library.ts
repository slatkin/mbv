// The Library sections as tests carry them (gui-change-driven-sections design
// D8): the core's one pass builds `libraryList` and `libraryDetail` together,
// and the right-click menu rides its own `rowMenu` section. A test writes the
// stores through one call. Tests only; production code imports nothing from
// here.

import { libraryDetail, libraryList, rowMenu } from './sections';
import type { GuiLibraryDetail, GuiLibraryList, GuiLibraryRowMenu } from './types';

/** One Library pass's list part, detail part, and the menu the pass carries. */
export interface LibrarySections {
	list: GuiLibraryList;
	detail: GuiLibraryDetail;
	/** The right-click menu the window holds; `null` when a test carries none. */
	rowMenu?: GuiLibraryRowMenu | null;
}

/** Carry the Library sections, or clear them with `null` (no presented
 * destination is active, so the menu clears with them). */
export function carryLibrary(sections: LibrarySections | null): void {
	libraryList.set(sections?.list ?? null);
	libraryDetail.set(sections?.detail ?? null);
	rowMenu.set(sections?.rowMenu ?? null);
}
