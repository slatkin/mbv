# Emby count fields: documentation findings

Sources are public Emby documentation and community threads; the Emby server is closed source. No live calls were made against the user's server. Statements marked **Inferred** are reasoning from the evidence, not documented behavior.

## 1. The count fields are opt-in and thinly documented

`ChildCount` and `RecursiveItemCount` are opt-in `ItemFields` values; the enum page gives no description of either. [ItemFields enum](https://dev.emby.media/reference/pluginapi/MediaBrowser.Model.Querying.ItemFields.html). A 2017 thread reports `ChildCount` missing from responses until it is named in `Fields` ([3219 bug](https://emby.media/community/topic/48053-3219-bug)). A user reported that `Fields=ChildCount` returned nothing on the generic `/Items` route and was told to use `/Users/{id}/Items` instead ([SeasonCount](https://emby.media/community/topic/84828-seasoncount/)); the user-scoped route is the reliable one. Staff have said the counts are computed on the fly and are left out of default output for speed ([thread 45019](https://emby.media/community/topic/45019-moviecountseriescountrecursiveitemcount)). Folder-type coverage is undocumented; a Kodi client log sends both fields together with `ParentId` and `Recursive=False` ([thread 79233](https://emby.media/community/topic/79233-emby-api)). mbv keeps `child_count` as `Option` (`crates/mbv-emby/src/types_parsing.rs:317`), so an absent field parses as `None`.

## 2. No official definition of what the counts include

No official source defines either count. Community evidence: for a Series, `RecursiveItemCount` is seasons plus episodes, and `RecursiveItemCount - ChildCount` gives the episode count — which implies `ChildCount` is direct children only and `RecursiveItemCount` includes sub-folders ([SeasonCount](https://emby.media/community/topic/84828-seasoncount/)). Inferred: both counts are type-agnostic, per-item properties unaffected by `IncludeItemTypes`, and watched items are counted.

## 3. A positive recursive count cannot prove a folder holds a video

Inferred: `RecursiveItemCount` can be positive while the folder holds no Video items (only empty sub-folders, or only audio/photos). mbv's existing empty-folder filter is `child_count == Some(0)` at `src/app/dispatch/library/browse/loading.rs:61`; it cannot prove a folder holds a video either.

## 4. A filtered count-only query returns a reliable total

`GET /Users/{id}/Items?ParentId=X&Recursive=true&IncludeItemTypes=Video&Limit=1` returns the filtered total in `TotalRecordCount` ([thread 115217](https://emby.media/community/topic/115217-api-to-extract-library-count)); [thread 19690](https://emby.media/community/topic/19690-incorrect-total-record-count-listing/) shows the total is independent of page size. The API reference does not document `EnableTotalRecordCount` or its default ([reference](https://dev.emby.media/reference/RestAPI/ItemsService/getUsersByUseridItems.html)), so send `EnableTotalRecordCount=true` explicitly. `Video` as an `IncludeItemTypes` value: check it against the item types the user's libraries actually use (`MediaTypes=Video` is an alternative filter) — implementation caveat for task 2.1.

## 5. Caveats of the count-only query

One request per root folder (run them in parallel, or skip folders where `ChildCount == Some(0)`); `Limit=1` still makes the server count the full matching set; parental controls change results per user, which is why the user-scoped route is used.

## Where mbv already requests and parses the counts

`crates/mbv-emby-model/src/lib.rs:106`; `crates/mbv-emby/src/types_parsing.rs:307-317`; `crates/mbv-emby/src/client_library.rs:169,392`; `crates/mbv-emby/src/client_playlists.rs:241`; `crates/mbv-emby/tests/emby/parsing.rs:28-31`; `src/app/dispatch/library/browse/loading.rs:61`.

## D1 choice

**(b) one count-only query per root folder** — `GET /Users/{UserId}/Items?ParentId=<folder>&Recursive=true&IncludeItemTypes=Video&Limit=1&EnableTotalRecordCount=true`, read `TotalRecordCount > 0` (user-scoped route; no `IsPlayed` filter, so watched videos count). Reason: no source defines what `RecursiveItemCount` counts; it likely counts sub-folders and all item types, so it cannot prove a folder holds a video.
