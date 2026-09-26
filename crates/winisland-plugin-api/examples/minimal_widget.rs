use winisland_plugin_api::sdk::*;

struct NowPlaying {
    widget: Widget,
    title: String,
    cover: Option<ImageHandle>,
}

impl NowPlaying {
    fn new(host: &Host) -> Result<Self, Error> {
        let widget = host
            .widgets()?
            .create(WidgetSpec::new("now-playing").span(2, 1))?;
        Ok(Self {
            widget,
            title: String::new(),
            cover: None,
        })
    }

    fn on_tick(&mut self, host: &Host) -> Result<(), Error> {
        let media = host.media()?;
        self.title = media.title().unwrap_or_default();
        self.cover = host.images()?.album_art().ok();
        let (w, h) = self.widget.logical_size();
        let mut list = DrawListBuilder::new(Size::new(w, h));
        list.fill_round_rect(Rect::new(0.0, 0.0, w, h), 8.0, Rgba::from_argb(0x8000_0000));
        if let Some(cover) = &self.cover {
            list.image(
                cover.id(),
                Rect::new(4.0, 4.0, h - 8.0, h - 8.0),
                ImageFit::Cover,
            );
        }
        list.text(
            &self.title,
            Rect::new(h + 2.0, 6.0, w - h - 6.0, h - 12.0),
            &TextStyle::default_at(13.0),
            Rgba::WHITE,
        );
        self.widget.submit(list.finish())
    }
}

fn main() {
    let _ = NowPlaying::new as fn(&Host) -> Result<NowPlaying, Error>;
    let _ = NowPlaying::on_tick as fn(&mut NowPlaying, &Host) -> Result<(), Error>;
}
