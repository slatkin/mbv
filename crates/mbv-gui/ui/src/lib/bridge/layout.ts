// The layout reporter (design D4): every mounted surface reports its list
// capacities and painted regions here, and the reporter sends the single
// merged `LayoutReport` the core expects — the core rebuilds its whole hit
// map from each report, so a surface must never send a partial one. Reports
// coalesce to one send per turn of the event loop, and a contribution that
// repeats its last value sends nothing: a frame that moves only the cursor
// moves no region, so it reports nothing (design D4: a report goes out after
// each paint that moved a region, no more).

import { sendLayoutReport } from './input';
import type { LayoutRegion, ListRows } from './types';

/** One surface's share of the merged report: its list capacities and regions. */
export interface LayoutContribution {
	lists: ListRows[];
	regions: LayoutRegion[];
}

const contributions = new Map<symbol, LayoutContribution>();
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
		void sendLayoutReport({ lists, regions, width: window.innerWidth });
	});
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
