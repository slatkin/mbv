// The Podcasts list's row model (add-gui-library-panel design D12, the
// USER-APPROVED GUI Podcasts design, row 10.1): the active pill's flat
// episode rows — no letter headings, because the list is sorted by date, and
// so no letter rail. The list paints fixed-height rows over native scroll and
// only the rows in view, as the Shows and Home lists do, and the model is
// plain data over fixed heights, so the geometry is asserted without a
// layout, which jsdom does not have.

import type { GuiPodcastsRow } from '$lib/bridge/types';
import { buildFlatRows } from './list';
import type { FlatRow } from './list';

/** The episode row's height in logical pixels (design D12): the room a
 * two-line episode title and the date gutter paint into, as a Shows flat
 * row. */
export const PODCASTS_ROW_HEIGHT = 48;

/** One painted row of the Podcasts list (design D12). The list is one flat
 * episode list in core order — newest first with undated episodes last — so
 * every painted row is an item row: the `id` is the row's stable identity as
 * one opaque string, the cursor's identity, the each-block's key, and the id
 * the scroll geometry reads. There is no heading and no rail to step over. */
export type PodcastsItemRow = FlatRow<GuiPodcastsRow>;

/**
 * The painted rows of the Podcasts list (design D12): the frame's episode
 * rows at one height, with no heading and no letter, in the core's order —
 * the latest pill's merged newest-first order, or a show's own episodes. The
 * filter narrows the rows to the titles that contain its text, without case
 * (design D7's filter rule). Every row carries its top edge and height, so
 * the list paints a window of rows over native scroll.
 */
export function buildPodcastsRows(
	rows: readonly GuiPodcastsRow[],
	filter = '',
): PodcastsItemRow[] {
	return buildFlatRows(rows, PODCASTS_ROW_HEIGHT, filter);
}
