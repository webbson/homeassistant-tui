use std::sync::Arc;
use std::time::Duration;

use indexmap::IndexMap;
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinHandle;

use crate::config::Alias;
use crate::ha::{HaCommand, InstanceRuntime};

pub struct InstanceHandle {
    pub command_tx: mpsc::UnboundedSender<HaCommand>,
    pub shutdown: Arc<Notify>,
    pub join: JoinHandle<()>,
}

pub struct InstanceRegistry {
    pub runtimes: IndexMap<Alias, InstanceRuntime>,
    pub handles: IndexMap<Alias, InstanceHandle>,
}

impl InstanceRegistry {
    pub fn new() -> Self {
        Self {
            runtimes: IndexMap::new(),
            handles: IndexMap::new(),
        }
    }

    pub fn add(&mut self, runtime: InstanceRuntime, handle: InstanceHandle) {
        let alias = runtime.alias.clone();
        self.runtimes.insert(alias.clone(), runtime);
        self.handles.insert(alias, handle);
    }

    pub fn get_mut(&mut self, alias: &str) -> Option<&mut InstanceRuntime> {
        self.runtimes.get_mut(alias)
    }

    pub fn send(&self, alias: &str, cmd: HaCommand) -> bool {
        if let Some(h) = self.handles.get(alias) {
            h.command_tx.send(cmd).is_ok()
        } else {
            false
        }
    }

    /// Non-blocking remove: signals shutdown and spawns a task to reap the
    /// join handle. Returns the runtime immediately for synchronous callers.
    pub fn remove_nowait(&mut self, alias: &str) -> Option<InstanceRuntime> {
        if let Some(handle) = self.handles.shift_remove(alias) {
            handle.shutdown.notify_waiters();
            tokio::spawn(async move {
                tokio::time::timeout(Duration::from_secs(2), handle.join)
                    .await
                    .ok();
            });
        }
        self.runtimes.shift_remove(alias)
    }
}

impl Default for InstanceRegistry {
    fn default() -> Self {
        Self::new()
    }
}
