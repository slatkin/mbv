mod row;
mod wide;

pub(in crate::app) use wide::render_wide_media_list_component;

#[cfg(test)]
mod wide_row_content_tests;
#[cfg(test)]
mod wide_row_geometry_tests;
#[cfg(test)]
mod wide_row_playback_tests;
#[cfg(test)]
mod wide_row_regression_tests_helpers;
#[cfg(test)]
mod wide_row_title_tests;
