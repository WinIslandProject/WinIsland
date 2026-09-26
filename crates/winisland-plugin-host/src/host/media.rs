use super::PluginHost;

#[derive(Clone)]
pub struct MediaSnapshot {
    pub resource_id: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub is_playing: bool,
    pub duration_ms: u64,
    pub position_ms: u64,
    pub available_controls: u32,
    pub cover: Vec<u8>,
}

impl PluginHost {
    pub fn dispatch_media_command(
        &self,
        resource_id: u64,
        command: u32,
        position_ms: u64,
    ) -> Result<(), crate::PluginHostError> {
        use crate::resources::ResourceKind;
        let token = self
            .runtime
            .resources
            .owner(ResourceKind::Media, resource_id)
            .map_err(|_| crate::PluginHostError::Invalid("media resource is stale".into()))?;
        let entries = self.entries.borrow();
        let instance = entries
            .iter()
            .find(|entry| entry.token() == token)
            .ok_or_else(|| crate::PluginHostError::Invalid("media owner is unavailable".into()))?;
        instance.queue_media_command(resource_id, command, position_ms)
    }

    pub fn media_snapshot(&self) -> Option<(u64, Option<MediaSnapshot>)> {
        let state = self.runtime.state.lock().ok()?;
        let source = state.media.iter().max_by_key(|(_, media)| media.sequence);
        Some((
            state.media_revision,
            source.map(|(id, media)| MediaSnapshot {
                resource_id: *id,
                title: media.title.clone(),
                artist: media.artist.clone(),
                album: media.album.clone(),
                is_playing: media.flags
                    & winisland_plugin_api::types::v2::context::MEDIA_FLAG_PLAYING
                    != 0,
                duration_ms: media.duration_ms,
                position_ms: media.position_ms,
                available_controls: media.available_controls,
                cover: media.cover.clone(),
            }),
        ))
    }
}
