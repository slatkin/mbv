// The layout reporter (design D4): every mounted surface reports its list
// capacities and painted regions here, and the reporter sends the single
// merged `LayoutReport` the core expects — the core rebuilds its whole hit
// map from each report, so a surface must never send a partial one. Reports
// coalesce to one send per turn of the event loop, and a contribution that
// repeats its last value sends nothing: a frame that moves only the cursor
// moves no region, so it reports nothing (design D4: a report goes out after
// each paint that moved a region, no more).

import { sendLayoutReport } from './input';
import type { LayoutRegion, LayoutSize, ListRows } from './types';

/** One surface's share of the merged report: its list capacities and regions. */
export interface LayoutContribution {
	lists: ListRows[];
	regions: LayoutRegion[];
}

const contributions = new Map<symbol, LayoutContribution>();

/** The panel container's last reported painted size (design D10): the size
 * the container queries pick the panel state from. Zero while no container
 * has reported — the pre-attach states paint no panel. */
let containerSize: LayoutSize = { width: 0, height: 0 };

let scheduled = false;

function schedule(): void {
	if (scheduled) {
		return;
	}
	scheduled = true;
	queueMicrotask(() => {
		scheduled = false;
		const lists: ListRows[] = [];
		const regions: LayoutRegion[] = [];
		for (const contribution of contributions.values()) {
			lists.push(...contribution.lists);
			regions.push(...contribution.regions);
		}
		void sendLayoutReport({ lists, regions, containerSize });
	});
}

/**
 * Records the panel container's painted size and schedules the merged
 * report (design D10): the container's own ResizeObserver calls this, and
 * the report carries the size so the capacity and regions it merges stay
 * matched to the layout the size painted. An unchanged size is a no-op.
 */
export function reportContainerSize(size: LayoutSize): void {
	if (size.width === containerSize.width && size.height === containerSize.height) {
		return;
	}
	containerSize = size;
	schedule();
}

/**
 * Registers or replaces one surface's contribution and schedules the merged
 * report. `key` is a symbol the caller holds for the surface's life (one per
 * component instance). A contribution identical to the last one reported
 * under the key is a no-op.
 */
export function reportLayout(key: symbol, contribution: LayoutContribution): void {
	const last = contributions.get(key);
	if (last && JSON.stringify(last) === JSON.stringify(contribution)) {
		return;
	}
	contributions.set(key, contribution);
	schedule();
}

/**
 * Removes a surface's contribution: an unmounted surface owns no regions, so
 * the next merged report drops them. Schedules a report only when the key
 * still had one.
 */
export function retractLayout(key: symbol): void {
	if (contributions.delete(key)) {
		schedule();
	}
}
