use std::collections::HashMap;
use std::sync::Mutex;

use winisland_plugin_api::abi::PluginStatus;
use winisland_plugin_api::types::v2::PluginToken;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ResourceKind {
    Context,
    Media,
    I18n,
    Widget,
    Lyrics,
    Settings,
    Image,
    HostStateSubscription,
}

#[derive(Clone, Copy)]
struct Owner {
    token: PluginToken,
    kind: ResourceKind,
    bytes: usize,
}

struct ResourceState {
    next: u64,
    owners: HashMap<u64, Owner>,
}

pub struct ResourceTable {
    inner: Mutex<ResourceState>,
}

fn limits(kind: ResourceKind) -> (usize, usize) {
    match kind {
        ResourceKind::Context => (64, usize::MAX),
        ResourceKind::Media => (4, 32 * 1024 * 1024),
        ResourceKind::I18n => (16, 4 * 1024 * 1024),
        ResourceKind::Widget => (8, 4 * 1024 * 1024),
        ResourceKind::Lyrics => (4, usize::MAX),
        ResourceKind::Settings => (1, 2 * 1024 * 1024),
        ResourceKind::Image => (64, 64 * 1024 * 1024),
        ResourceKind::HostStateSubscription => (16, usize::MAX),
    }
}

impl Default for ResourceTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceTable {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(ResourceState {
                next: 1_u64 << 32,
                owners: HashMap::new(),
            }),
        }
    }

    pub fn allocate(
        &self,
        token: PluginToken,
        kind: ResourceKind,
        bytes: usize,
    ) -> Result<u64, PluginStatus> {
        if token == PluginToken::INVALID {
            return Err(PluginStatus::StaleHandle);
        }
        let mut state = self.inner.lock().map_err(|_| PluginStatus::Internal)?;
        let (max_count, max_bytes) = limits(kind);
        let mut count = 0_usize;
        let mut used_bytes = 0_usize;
        for owner in state.owners.values() {
            if owner.token == token && owner.kind == kind {
                count += 1;
                used_bytes = used_bytes
                    .checked_add(owner.bytes)
                    .ok_or(PluginStatus::LimitExceeded)?;
            }
        }
        if count >= max_count || bytes > max_bytes.saturating_sub(used_bytes) {
            return Err(PluginStatus::LimitExceeded);
        }
        let id = state.next;
        state.next = state
            .next
            .checked_add(1)
            .ok_or(PluginStatus::LimitExceeded)?;
        state.owners.insert(id, Owner { token, kind, bytes });
        Ok(id)
    }

    pub fn require(
        &self,
        token: PluginToken,
        kind: ResourceKind,
        id: u64,
    ) -> Result<(), PluginStatus> {
        let state = self.inner.lock().map_err(|_| PluginStatus::Internal)?;
        match state.owners.get(&id) {
            Some(owner) if owner.token == token && owner.kind == kind => Ok(()),
            _ => Err(PluginStatus::StaleHandle),
        }
    }

    pub fn list(&self, token: PluginToken, kind: ResourceKind) -> Result<Vec<u64>, PluginStatus> {
        let state = self.inner.lock().map_err(|_| PluginStatus::Internal)?;
        Ok(state
            .owners
            .iter()
            .filter_map(|(id, owner)| (owner.token == token && owner.kind == kind).then_some(*id))
            .collect())
    }

    pub fn owner(&self, kind: ResourceKind, id: u64) -> Result<PluginToken, PluginStatus> {
        let state = self.inner.lock().map_err(|_| PluginStatus::Internal)?;
        match state.owners.get(&id) {
            Some(owner) if owner.kind == kind => Ok(owner.token),
            _ => Err(PluginStatus::StaleHandle),
        }
    }

    pub fn resize(
        &self,
        token: PluginToken,
        kind: ResourceKind,
        id: u64,
        bytes: usize,
    ) -> Result<(), PluginStatus> {
        let mut state = self.inner.lock().map_err(|_| PluginStatus::Internal)?;
        let Some(owner) = state.owners.get(&id).copied() else {
            return Err(PluginStatus::StaleHandle);
        };
        if owner.token != token || owner.kind != kind {
            return Err(PluginStatus::StaleHandle);
        }
        let (_, max_bytes) = limits(kind);
        let used_other = state
            .owners
            .iter()
            .filter(|(resource_id, entry)| {
                **resource_id != id && entry.token == token && entry.kind == kind
            })
            .try_fold(0_usize, |sum, (_, entry)| sum.checked_add(entry.bytes))
            .ok_or(PluginStatus::LimitExceeded)?;
        if bytes > max_bytes.saturating_sub(used_other) {
            return Err(PluginStatus::LimitExceeded);
        }
        if let Some(owner) = state.owners.get_mut(&id) {
            owner.bytes = bytes;
        }
        Ok(())
    }

    pub fn release(
        &self,
        token: PluginToken,
        kind: ResourceKind,
        id: u64,
    ) -> Result<(), PluginStatus> {
        let mut state = self.inner.lock().map_err(|_| PluginStatus::Internal)?;
        match state.owners.get(&id) {
            Some(owner) if owner.token == token && owner.kind == kind => {
                state.owners.remove(&id);
                Ok(())
            }
            _ => Err(PluginStatus::StaleHandle),
        }
    }

    pub fn revoke_plugin(
        &self,
        token: PluginToken,
    ) -> Result<Vec<(u64, ResourceKind)>, PluginStatus> {
        let mut state = self.inner.lock().map_err(|_| PluginStatus::Internal)?;
        let mut revoked = Vec::new();
        state.owners.retain(|id, owner| {
            if owner.token == token {
                revoked.push((*id, owner.kind));
                false
            } else {
                true
            }
        });
        Ok(revoked)
    }
}
