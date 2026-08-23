use crate::core::persistence::{load_config, save_config};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager,
};

pub(super) fn auto_allow_new_apps(
    mgr: &GlobalSystemMediaTransportControlsSessionManager,
    allowed: &[String],
    known_apps: &mut Vec<String>,
) -> Vec<String> {
    let mut new_allowed = allowed.to_vec();
    let mut new_app_ids: Vec<String> = Vec::new();
    if let Ok(sessions) = mgr.GetSessions()
        && let Ok(count) = sessions.Size()
    {
        for i in 0..count {
            if let Ok(session) = sessions.GetAt(i)
                && let Ok(pb_info) = session.GetPlaybackInfo()
                && let Ok(playback_type) = pb_info.PlaybackType()
                && let Ok(value) = playback_type.Value()
                && value == windows::Media::MediaPlaybackType::Music
                && let Ok(id) = session.SourceAppUserModelId()
            {
                let app_id = id.to_string();
                if !new_app_ids.contains(&app_id) {
                    new_app_ids.push(app_id);
                }
            }
        }
    }

    if new_app_ids.is_empty() {
        return new_allowed;
    }
    if new_app_ids.iter().all(|app_id| known_apps.contains(app_id)) {
        return new_allowed;
    }

    let mut config = load_config();
    let mut changed = false;

    for app_id in &new_app_ids {
        if !config.smtc_known_apps.contains(app_id) {
            let is_first_run = config.smtc_known_apps.is_empty();
            config.smtc_known_apps.push(app_id.clone());

            if is_first_run && !config.smtc_apps.contains(app_id) {
                config.smtc_apps.push(app_id.clone());
                if !new_allowed.contains(app_id) {
                    new_allowed.push(app_id.clone());
                }
            }
            changed = true;
        }
    }

    if changed {
        save_config(&config);
        log::info!("SMTC: recorded new media session(s): {:?}", new_app_ids);
    }
    known_apps.clone_from(&config.smtc_known_apps);

    new_allowed
}

pub(super) fn get_target_session(
    mgr: &GlobalSystemMediaTransportControlsSessionManager,
    allowed: &[String],
) -> Option<GlobalSystemMediaTransportControlsSession> {
    if allowed.is_empty() {
        return None;
    }

    let current_session = mgr
        .GetCurrentSession()
        .ok()
        .filter(|session| session_is_allowed_music(session, allowed));
    if current_session.as_ref().is_some_and(session_is_playing) {
        return current_session;
    }

    let mut fallback_session = None;
    if let Ok(sessions) = mgr.GetSessions()
        && let Ok(count) = sessions.Size()
    {
        for i in 0..count {
            let Ok(session) = sessions.GetAt(i) else {
                continue;
            };
            if !session_is_allowed_music(&session, allowed) {
                continue;
            }
            if session_is_playing(&session) {
                return Some(session);
            }
            if fallback_session.is_none() {
                fallback_session = Some(session);
            }
        }
    }

    current_session.or(fallback_session)
}

fn session_is_allowed_music(
    session: &GlobalSystemMediaTransportControlsSession,
    allowed: &[String],
) -> bool {
    session.SourceAppUserModelId().is_ok_and(|id| {
        let app_id = id.to_string();
        allowed.iter().any(|allowed_id| allowed_id == &app_id)
    }) && is_music_session(session)
}

fn session_is_playing(session: &GlobalSystemMediaTransportControlsSession) -> bool {
    session.GetPlaybackInfo().is_ok_and(|info| {
        info.PlaybackStatus().is_ok_and(|status| {
            status
                == windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing
        })
    })
}

pub(super) fn is_music_session(session: &GlobalSystemMediaTransportControlsSession) -> bool {
    if let Ok(pb_info) = session.GetPlaybackInfo()
        && let Ok(playback_type) = pb_info.PlaybackType()
        && let Ok(value) = playback_type.Value()
        && value == windows::Media::MediaPlaybackType::Video
    {
        return false;
    }
    true
}
