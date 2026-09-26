use std::sync::Arc;

use winisland_platform::{MediaContext, MediaSessionHandle};

use crate::core::persistence::{load_config, save_config};

pub(super) fn auto_allow_new_apps(
    manager: &dyn MediaContext,
    allowed: &[String],
    known_apps: &mut Vec<String>,
) -> Vec<String> {
    let mut new_allowed = allowed.to_vec();
    let mut new_app_ids = Vec::new();
    for session in manager.sessions() {
        if session.is_music()
            && let Some(app_id) = session.source_app_id()
            && !new_app_ids.contains(&app_id)
        {
            new_app_ids.push(app_id);
        }
    }
    if new_app_ids.is_empty() || new_app_ids.iter().all(|app_id| known_apps.contains(app_id)) {
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
        log::info!("SMTC: recorded new media session(s): {new_app_ids:?}");
    }
    known_apps.clone_from(&config.smtc_known_apps);
    new_allowed
}

pub(super) fn get_target_session(
    manager: &dyn MediaContext,
    allowed: &[String],
) -> Option<Arc<dyn MediaSessionHandle>> {
    if allowed.is_empty() {
        return None;
    }
    let current_session = manager
        .current()
        .filter(|session| session_is_allowed_music(session.as_ref(), allowed));
    if current_session
        .as_ref()
        .is_some_and(|session| session.is_playing())
    {
        return current_session;
    }
    let mut fallback_session = None;
    for session in manager.sessions() {
        if !session_is_allowed_music(session.as_ref(), allowed) {
            continue;
        }
        if session.is_playing() {
            return Some(session);
        }
        if fallback_session.is_none() {
            fallback_session = Some(session);
        }
    }
    current_session.or(fallback_session)
}

fn session_is_allowed_music(session: &dyn MediaSessionHandle, allowed: &[String]) -> bool {
    session
        .source_app_id()
        .is_some_and(|app_id| allowed.contains(&app_id))
        && !session.is_video()
}
