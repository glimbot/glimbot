use std::sync::atomic::{AtomicU64, Ordering};
use parking_lot::{RwLock, RwLockUpgradableReadGuard};
use nom::lib::std::collections::HashMap;

#[derive(Debug)]
pub(crate) struct CommandStats {
    times_used: AtomicU64,
}

impl Clone for CommandStats {
    fn clone(&self) -> Self {
        CommandStats {
            times_used: AtomicU64::new(self.times_used.load(Ordering::Relaxed))
        }
    }
}

#[derive(Debug)]
pub(crate) struct GuildStats {
    commands_used: RwLock<HashMap<String, CommandStats>>,
}

impl CommandStats {
    pub fn new() -> CommandStats {
        CommandStats {
            times_used: AtomicU64::new(0)
        }
    }

    pub fn incr(&self) -> u64 {
        self.times_used.fetch_add(1, Ordering::Relaxed)
    }
}

impl GuildStats {
    pub fn new() -> GuildStats {
        GuildStats {
            commands_used: RwLock::new(HashMap::new())
        }
    }

    pub fn add_usage(&self, cmd: impl AsRef<str>) -> u64{
        let cmd = cmd.as_ref();
        let rg = self.commands_used.upgradable_read();
        if !rg.contains_key(cmd) {
            let mut wg = RwLockUpgradableReadGuard::upgrade(rg);
            wg.insert(cmd.to_string(), CommandStats::new());
            wg.get(cmd).unwrap().incr() + 1
        } else {
            rg.get(cmd).unwrap().incr() + 1
        }
    }

    pub fn cur_stats(&self) -> HashMap<String, CommandStats> {
        self.commands_used.read().clone()
    }
}
