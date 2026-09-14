# rstest conversion baseline

Captured before any test conversion, after adding the `rstest` dependency.
Counts come from `cargo nextest list` filtered to the measured families.

## Socket `*_returns_none` family (8)

Command:

```text
cargo nextest list -p mbv-core | grep -E 'audiobookshelf_socket::tests::(malformed_json|truncated_open_packet|engine_(ping|pong|close)|message_(disconnect|ack|connect_error))_returns_none'
```

Test names:

```text
mbv-core audiobookshelf_socket::tests::engine_close_returns_none
mbv-core audiobookshelf_socket::tests::engine_ping_returns_none
mbv-core audiobookshelf_socket::tests::engine_pong_returns_none
mbv-core audiobookshelf_socket::tests::malformed_json_returns_none
mbv-core audiobookshelf_socket::tests::message_ack_returns_none
mbv-core audiobookshelf_socket::tests::message_connect_error_returns_none
mbv-core audiobookshelf_socket::tests::message_disconnect_returns_none
mbv-core audiobookshelf_socket::tests::truncated_open_packet_returns_none
```

## `parse_item_*_is_folder` family (5)

Command:

```text
cargo nextest list -p mbv-core | grep 'api::tests::parse_item_.*_is_folder'
```

Test names:

```text
mbv-core api::tests::parse_item_channel_forces_is_folder
mbv-core api::tests::parse_item_collection_folder_forces_is_folder
mbv-core api::tests::parse_item_music_album_is_folder
mbv-core api::tests::parse_item_music_artist_is_folder
mbv-core api::tests::parse_item_series_is_folder
```

## `parse_video_info_*` family (4)

Command:

```text
cargo nextest list -p mbv-core | grep -E 'api::tests::parse_video_info_(1080p|4k|720p|codec_only_when_no_resolution)$'
```

Test names:

```text
mbv-core api::tests::parse_video_info_1080p
mbv-core api::tests::parse_video_info_4k
mbv-core api::tests::parse_video_info_720p
mbv-core api::tests::parse_video_info_codec_only_when_no_resolution
```

## Backdrop `dim_*` family (7)

Command:

```text
cargo nextest list -p mbv | grep 'backdrop::tests::dim_'
```

Test names:

```text
mbv::bin/mbv app::render::components::backdrop::tests::dim_black_stays_black
mbv::bin/mbv app::render::components::backdrop::tests::dim_indexed_passthrough
mbv::bin/mbv app::render::components::backdrop::tests::dim_reset_stays_black
mbv::bin/mbv app::render::components::backdrop::tests::dim_rgb_halves_each_channel
mbv::bin/mbv app::render::components::backdrop::tests::dim_rgb_max_becomes_half
mbv::bin/mbv app::render::components::backdrop::tests::dim_rgb_zero_stays_zero
mbv::bin/mbv app::render::components::backdrop::tests::dim_white_becomes_half_bright_gray
```

The adjacent socket `non_array_event_payload_returns_none` and parsing
`parse_video_info_empty_when_no_video_stream` tests are not part of the measured
families above.

## Build-time note

No pre-change wall-time baseline was recorded. Post-change-only measurements
using `cargo nextest run --release --test-threads=4`:

- Cold (after `cargo clean -p mbv -p mbv-core -p mbvd`): 19 seconds; 1960 tests passed.
- Warm: 19 seconds; 1960 tests passed.
