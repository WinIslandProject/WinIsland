use std::collections::HashSet;
use winisland_plugin_api::types::v2::context::HostStateV2;
use winisland_plugin_host::host::MediaSnapshot;

use super::App;

impl App {
    pub(super) fn update_v2_album_art(&self, cover: Option<&[u8]>, hash: u64) {
        let Some(host) = &self.plugin_host else {
            return;
        };
        if self.v2_album_art_hash.get() == Some(hash) {
            return;
        }
        let image = cover.and_then(winisland_render::Image::decode);
        if let Err(status) = host.runtime().set_album_art(image) {
            log::warn!("Cannot update ABI v2 album art: {status:?}");
            return;
        }
        self.v2_album_art_hash.set(Some(hash));
    }

    pub(super) fn refresh_v2_contexts(&mut self) {
        let Some((revision, contexts)) = self
            .plugin_host
            .as_ref()
            .and_then(|host| host.contexts_snapshot())
        else {
            return;
        };
        if revision == self.v2_context_revision {
            return;
        }
        let current_ids = contexts
            .iter()
            .map(|context| context.id)
            .collect::<HashSet<_>>();
        for id in self.v2_context_ids.difference(&current_ids) {
            self.ctx_mgr.remove_context(*id);
        }
        for context in contexts {
            self.ctx_mgr.upsert_context(context);
        }
        self.v2_context_ids = current_ids;
        self.v2_context_revision = revision;
    }

    pub(super) fn next_v2_settings_pages(
        &mut self,
    ) -> Option<Vec<winisland_core::plugin_settings::PluginSettingsPage>> {
        let (revision, pages) = self.plugin_host.as_ref()?.settings_pages_snapshot()?;
        if revision == self.v2_settings_revision {
            return None;
        }
        self.v2_settings_revision = revision;
        Some(pages)
    }

    pub(super) fn next_v2_media_event(&mut self) -> Option<Option<MediaSnapshot>> {
        let (revision, source) = self.plugin_host.as_ref()?.media_snapshot()?;
        if revision == self.v2_media_revision {
            return None;
        }
        self.v2_media_revision = revision;
        Some(source)
    }

    pub(super) fn update_v2_host_state(&self, title: &str, artist: &str, playing: bool) {
        let Some(host) = &self.plugin_host else {
            return;
        };
        let mut state = HostStateV2::default();
        copy_text(&mut state.media_title, title);
        copy_text(&mut state.media_artist, artist);
        state.is_playing = u8::from(playing);
        copy_text(
            &mut state.theme,
            if self.is_light_theme { "light" } else { "dark" },
        );
        if let Err(error) = host.set_host_state(state) {
            log::warn!("Cannot update ABI v2 host state: {error}");
        }
    }

    pub(super) fn refresh_v2_widgets(&mut self) -> bool {
        let Some(host) = &self.plugin_host else {
            return false;
        };
        for error in host.drain_failed_plugins() {
            log::warn!("{error}");
        }
        let widgets = host.widgets_snapshot();
        let current_ids = widgets
            .iter()
            .map(|widget| widget.id)
            .collect::<HashSet<_>>();
        let mut changed = false;
        for id in self.v2_widget_ids.difference(&current_ids) {
            changed |= self.widget_mgr.remove_widget(*id);
            self.plugin_frames.remove(id);
        }
        for widget in widgets {
            let previous = self
                .widget_mgr
                .widgets()
                .iter()
                .find(|old| old.id == widget.id);
            let needs_update = previous.is_none_or(|old| {
                old.plugin_id != widget.plugin_id
                    || old.key != widget.key
                    || old.span_cols != widget.span_cols
                    || old.span_rows != widget.span_rows
                    || old.title != widget.title
                    || old.body != widget.body
            });
            if needs_update {
                self.widget_mgr.upsert_widget(widget);
                changed = true;
            }
        }
        self.v2_widget_ids = current_ids;
        if changed {
            for error in host.refresh_tick_widgets() {
                log::warn!("{error}");
            }
        }
        changed
    }

    pub(super) fn prepare_v2_frames(&mut self) {
        let Some(host) = &self.plugin_host else {
            return;
        };
        for id in &self.v2_widget_ids {
            match host.prepare_widget_frame(*id) {
                Ok(Some(frame)) => {
                    self.plugin_frames.insert(*id, frame);
                }
                Ok(None) => {}
                Err(error) if error.consecutive_failures == 1 || error.disabled => {
                    log::warn!("Plugin widget {id} draw list rejected: {}", error.reason);
                    if error.disabled {
                        self.plugin_frames.remove(id);
                    }
                }
                Err(_) => {}
            }
        }
    }
}

fn copy_text(target: &mut [u8], value: &str) {
    let mut len = value.len().min(target.len().saturating_sub(1));
    while !value.is_char_boundary(len) {
        len -= 1;
    }
    target[..len].copy_from_slice(&value.as_bytes()[..len]);
}
