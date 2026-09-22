# Row 8.2 — manual check: `/Shows/Upcoming` scoped to a TV library

Date: 2026-09-22. Server: the user's Emby (configured in mbv as
`http://192.168.10.6:8096`, public alias `https://emby.poo.town`), user
`52a516340de04827b9c76b2c690a957e`, authenticated with mbv's stored token and
mbv's exact header form (`Authorization: Emby Client=... Token=...` +
`X-Emby-Token`).

## Observed behavior

- TV library id `9` ("TV shows"). Request mirrors mbv's `get_upcoming`:
  `GET /Shows/Upcoming?ParentId=9&Limit=30&UserId=...` (with and without the
  `Fields` param — same result).
- `TotalRecordCount: 53`, 30 returned. All rows are `Type: Episode` with
  series/episode names and `PremiereDate` — the row shape is usable.
- **Every returned row has `LocationType: Virtual` and no `Id` field and no
  media sources.** Virtual items are unaired/not-yet-downloaded placeholders;
  without an `Id` mbv cannot build a stable target or play them. The row shape
  is therefore **not playable as returned** on this server today.

## Filtering needed (follow-up)

- To present only playable rows, filter client-side on
  `LocationType != "Virtual"` (and require a present `Id`); `/Shows/Upcoming`
  accepts no server-side filter for this. On this server that currently yields
  **zero** rows — the upcoming slate is entirely virtual — so a filtered
  Upcoming mode would render empty until episodes are actually downloaded.
- Unfiltered (current implementation): rows render and cycle/selection work,
  but activation of a virtual row cannot play. Follow-up option if desired:
  hide or visually mark `Virtual` rows in the Upcoming mode. This is a product
  decision outside the rework-tv-library-pills change scope; recorded here per
  row 8.2's "or record the filtering needed" branch.

Note: the endpoint is slow on this server (~23 s for the scoped query); a
smaller `Limit` did not change the latency materially.
