// The row memo the Library lists share (gui-change-driven-sections design D9).
// A list's painted rows are a pure function of its carried content and its
// interaction facts: the filter text, the open nodes, the view mode. A scroll
// step or a cursor move reads the same facts, so the memo hands back the rows
// it built last, and the rows keep their identity. Each list keeps its own memo.

/** Wrap `build` so a call with the arguments it last saw returns its rows
 * again. The memo keeps one slot, because a list paints one content at a time,
 * and it compares the arguments by identity: a caller passes the carried
 * content object and the state's values, never a fresh object. */
export function memoRows<Args extends readonly unknown[], Row>(
	build: (...args: Args) => Row[],
): (...args: Args) => Row[] {
	let last: { args: Args; rows: Row[] } | null = null;
	return (...args: Args): Row[] => {
		if (last !== null && sameArgs(last.args, args)) {
			return last.rows;
		}
		const rows = build(...args);
		last = { args, rows };
		return rows;
	};
}

function sameArgs(a: readonly unknown[], b: readonly unknown[]): boolean {
	return a.length === b.length && a.every((value, index) => value === b[index]);
}
