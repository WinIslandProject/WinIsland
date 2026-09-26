use std::collections::HashSet;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use winisland_core::lyrics::LyricLine;
use winisland_plugin_api::abi::PluginStatus;
use winisland_plugin_api::types::v2::lyrics::{LYRICS_TEXT_FLAG_WORD_SYNCED, LyricsTextV2};
use winisland_plugin_api::types::v2::{PluginToken, ResourceId, Utf8Slice};

use super::{PluginCallGuard, PluginIdentity, PluginInstance, WorkerEvent};
use crate::resources::ResourceKind;
use crate::runtime::HostRuntime;

#[derive(Clone, Default)]
pub struct LyricsBridge {
    workers: Arc<Mutex<Vec<LyricsWorker>>>,
}

#[derive(Clone)]
struct LyricsWorker {
    id: String,
    token: PluginToken,
    sender: Sender<WorkerEvent>,
}

impl LyricsBridge {
    pub(crate) fn attach(&self, instance: &PluginInstance) {
        if let Some(sender) = &instance.event_tx
            && let Ok(mut workers) = self.workers.lock()
        {
            workers.push(LyricsWorker {
                id: instance.library().metadata().id.clone(),
                token: instance.token(),
                sender: sender.clone(),
            });
        }
    }

    pub(crate) fn detach(&self, plugin_id: &str) {
        if let Ok(mut workers) = self.workers.lock() {
            workers.retain(|worker| worker.id != plugin_id);
        }
    }

    pub fn apply(&self, mut lyrics: Arc<Vec<LyricLine>>) -> Arc<Vec<LyricLine>> {
        let Ok(guard) = self.workers.lock() else {
            return lyrics;
        };
        let mut workers = guard
            .iter()
            .cloned()
            .map(|worker| (worker, true))
            .collect::<Vec<_>>();
        drop(guard);
        if workers.is_empty() {
            return lyrics;
        }
        let mut reported = HashSet::new();
        for line in Arc::make_mut(&mut lyrics) {
            for (worker, active) in &mut workers {
                if !*active {
                    continue;
                }
                let (response, receive) = mpsc::channel();
                let event = WorkerEvent::LyricsTransform {
                    token: worker.token,
                    time_ms: line.time_ms,
                    word_synced: line.is_word_synced(),
                    text: line.text.clone(),
                    response,
                };
                let result = worker
                    .sender
                    .send(event)
                    .map_err(|_| PluginStatus::Internal)
                    .and_then(|_| {
                        receive
                            .recv_timeout(Duration::from_secs(2))
                            .map_err(|_| PluginStatus::Internal)?
                    });
                match result {
                    Ok(text) => {
                        if !line.replace_text_preserving_timings(text)
                            && reported.insert(worker.id.clone())
                        {
                            log::warn!(
                                "Lyrics transformer '{}' changed word timing boundaries",
                                worker.id
                            );
                        }
                    }
                    Err(status) => {
                        *active = false;
                        if reported.insert(worker.id.clone()) {
                            log::warn!("Lyrics transformer '{}' failed: {status:?}", worker.id);
                        }
                    }
                }
            }
        }
        lyrics
    }
}

pub(super) fn dispatch_transform(
    runtime: &HostRuntime,
    identity: &Arc<PluginIdentity>,
    token: PluginToken,
    time_ms: u64,
    word_synced: bool,
    mut text: String,
) -> Result<String, PluginStatus> {
    let mut ids = runtime.resources.list(token, ResourceKind::Lyrics)?;
    ids.sort_unstable();
    for id in ids {
        let (callback, callback_data) = {
            let mut state = runtime.state.lock().map_err(|_| PluginStatus::Internal)?;
            let record = state.lyrics.get_mut(&id).ok_or(PluginStatus::StaleHandle)?;
            record.in_flight = record.in_flight.saturating_add(1);
            (record.on_transform, record.callback_data)
        };
        let result = transform_one(
            identity,
            callback,
            callback_data,
            id,
            time_ms,
            word_synced,
            &text,
        );
        if let Ok(mut state) = runtime.state.lock()
            && let Some(record) = state.lyrics.get_mut(&id)
        {
            record.in_flight = record.in_flight.saturating_sub(1);
        }
        text = result?;
    }
    Ok(text)
}

fn transform_one(
    identity: &Arc<PluginIdentity>,
    callback: winisland_plugin_api::types::v2::lyrics::LyricsTransformFnV2,
    callback_data: usize,
    id: u64,
    time_ms: u64,
    word_synced: bool,
    text: &str,
) -> Result<String, PluginStatus> {
    let input = LyricsTextV2 {
        struct_size: std::mem::size_of::<LyricsTextV2>() as u32,
        flags: if word_synced {
            LYRICS_TEXT_FLAG_WORD_SYNCED
        } else {
            0
        },
        line_time_ms: time_ms,
        text: Utf8Slice::borrowed(text),
    };
    let id = unsafe { ResourceId::from_raw(id) };
    let mut required = 0u32;
    let _guard = PluginCallGuard::enter(identity);
    // SAFETY: The callback is leased and the input remains live for the size query.
    let status = unsafe {
        callback(
            callback_data as *mut _,
            id,
            &input,
            std::ptr::null_mut(),
            0,
            &mut required,
        )
    };
    if status != PluginStatus::Ok {
        return Err(status);
    }
    if required > 256 * 1024 {
        return Err(PluginStatus::LimitExceeded);
    }
    if required == 0 {
        return Ok(String::new());
    }
    let mut output = vec![0u8; required as usize];
    let mut written = required;
    // SAFETY: The callback is leased and the output buffer has the requested capacity.
    let status = unsafe {
        callback(
            callback_data as *mut _,
            id,
            &input,
            output.as_mut_ptr(),
            required,
            &mut written,
        )
    };
    if status != PluginStatus::Ok {
        return Err(status);
    }
    if written > required {
        return Err(PluginStatus::InvalidArgument);
    }
    output.truncate(written as usize);
    String::from_utf8(output).map_err(|_| PluginStatus::InvalidArgument)
}
