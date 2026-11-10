// The Search DTOs of the GUI bridge contract (openspec change
// add-gui-library-panel, design D12, row 12.1). Each type mirrors one serde
// DTO in crates/mbv-gui/src/bridge field for field: the names are the serde
// wire names, so a rename on the Rust side is a rename here.
// `library-types.ts` re-exports this family with the rest of the Library's
// wire types, so every call site keeps importing from the one bridge module.

/** The Search facts (design D12, row 12.1): the query the core is answering,
 * whether its fetch is still landing or failed, and the rows it found. A
 * frame carries `null` for the section while no search runs and no results
 * hold — no field text is up, or the last one closed. */
export interface GuiSearchFrame {
	/** The query the section answers, as the last `searchQuery` command
	 * carried it. */
	query: string;
	/** Whether the query's fetch has not landed yet. */
	loading: boolean;
	/** Whether the query's fetch failed. */
	failed: boolean;
	/** The found rows in the core's order. */
	results: GuiSearchRow[];
}

/** One found row (design D12, row 12.1). The identity is the row's stable
 * Service id an activate command sends, never a row position, and
 * `itemType` names the destination the activation opens. */
export interface GuiSearchRow {
	id: string;
	itemType: string;
	title: string;
	subtitle: string;
}

/** The Search commands of the Library command (design D12, row 12.1): the
 * query the field typing sends, an activated result, and the close. One union
 * rides `GuiLibraryCommand`, so the search commands go through the same
 * `library` command as the rest. */
export type GuiSearchLibraryCommand =
	| {
			/** Type the query (row 12.1): the debounced send the typing
			 * makes. An empty query clears the results. */
			command: 'searchQuery';
			query: string;
	  }
	| {
			/** Activate the found row (row 12.1): opens the row through the
			 * core's activate path. */
			command: 'searchActivate';
			id: string;
			itemType: string;
	  }
	| {
			/** Close the results (row 12.1): the Esc that closes the field
			 * clears the section. */
			command: 'searchClose';
	  };
