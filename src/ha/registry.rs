use indexmap::IndexMap;
use tokio::sync::mpsc;

use crate::config::Alias;
use crate::ha::{HaCommand, InstanceRuntime};

pub struct InstanceRegistry {
    pub runtimes: IndexMap<Alias, InstanceRuntime>,
    pub command_tx: IndexMap<Alias, mpsc::UnboundedSender<HaCommand>>,
}

impl InstanceRegistry {
    pub fn new() -> Self {
        Self {
            runtimes: IndexMap::new(),
            command_tx: IndexMap::new(),
        }
    }

    pub fn add(&mut self, runtime: InstanceRuntime, tx: mpsc::UnboundedSender<HaCommand>) {
        self.command_tx.insert(runtime.alias.clone(), tx);
        self.runtimes.insert(runtime.alias.clone(), runtime);
    }

    pub fn get_mut(&mut self, alias: &str) -> Option<&mut InstanceRuntime> {
        self.runtimes.get_mut(alias)
    }

    #[allow(dead_code)]
    pub fn send(&self, alias: &str, cmd: HaCommand) -> bool {
        if let Some(tx) = self.command_tx.get(alias) {
            tx.send(cmd).is_ok()
        } else {
            false
        }
    }

    pub fn total_entities(&self) -> usize {
        self.runtimes.values().map(|r| r.states.len()).sum()
    }
}

impl Default for InstanceRegistry {
    fn default() -> Self {
        Self::new()
    }
}
