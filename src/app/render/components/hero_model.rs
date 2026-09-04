use mbv_core::api::EmbyItem;

/// Provider-neutral content exposed to the shared hero presentation.
pub(in crate::app) trait Hero {
    fn title(&self) -> &str;
    fn subtitle(&self) -> Option<&str>;
    fn meta_rows(&self, width: u16) -> Vec<String>;
    fn body(&self) -> HeroBody<'_>;
    fn artwork(&self) -> HeroArtwork<'_>;
}

pub(in crate::app) enum HeroBody<'a> {
    Listing(Vec<&'a str>),
    Description(&'a str),
}

pub(in crate::app) enum HeroArtwork<'a> {
    Image(&'a str),
    Placeholder,
}

impl Hero for EmbyItem {
    fn title(&self) -> &str {
        &self.name
    }

    fn subtitle(&self) -> Option<&str> {
        (!self.series_name.is_empty()).then_some(self.series_name.as_str())
    }

    fn meta_rows(&self, _width: u16) -> Vec<String> {
        let mut rows = Vec::new();
        if !self.premiere_date.is_empty() {
            rows.push(self.premiere_date.clone());
        }
        rows
    }

    fn body(&self) -> HeroBody<'_> {
        HeroBody::Description(&self.overview)
    }

    fn artwork(&self) -> HeroArtwork<'_> {
        if self.id.is_empty() {
            HeroArtwork::Placeholder
        } else {
            HeroArtwork::Image(&self.id)
        }
    }
}
