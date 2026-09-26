use winisland_platform::{BrightnessFeed, BrightnessSnapshot};

pub(super) struct BrightnessMonitor {
    feed: Option<Box<dyn BrightnessFeed>>,
}

impl BrightnessMonitor {
    pub(super) fn new() -> Self {
        let feed = match crate::platform::display().start_brightness_feed() {
            Ok(feed) => Some(feed),
            Err(error) => {
                log::warn!("Brightness monitor unavailable: {error}");
                None
            }
        };
        Self { feed }
    }

    pub(super) fn snapshot(&self) -> BrightnessSnapshot {
        self.feed
            .as_ref()
            .map(|feed| feed.snapshot())
            .unwrap_or_default()
    }

    pub(super) fn set_level(&self, level: f32) {
        if let Some(feed) = &self.feed {
            feed.set_level(level);
        }
    }
}
