use once_cell::sync::Lazy;
use crate::module::{ModInfo, Sensitivity, Module};
use std::collections::HashSet;
use std::iter::FromIterator;
use serenity::client::Context;
use crate::dispatch::{Dispatch, ShardManKey};
use serenity::model::channel::Message;
use std::time::{Instant, Duration};
use systemstat::Platform;
use serenity::builder::{CreateEmbed, CreateEmbedAuthor};
use crate::about::REPO_URL;
use serenity::utils::Color;
use std::sync::atomic::{AtomicU64, Ordering};

static STATUS_INFO: Lazy<ModInfo> = Lazy::new(|| {
    ModInfo {
        name: "bot-status",
        sensitivity: Sensitivity::Owner,
        does_filtering: true,
        command: true,
        config_values: Vec::new()
    }
});

pub const BYTES_IN_MIB: u64 = 1024 * 1024;

#[derive(Default)]
pub struct StatusModule {
    command_counter: AtomicU64
}

pub static START_TIME: Lazy<Instant> = Lazy::new(|| Instant::now());
pub const GLIM_COLOR: Color = Color::new(0xEDBBF3);

#[async_trait::async_trait]
impl Module for StatusModule {
    fn info(&self) -> &ModInfo {
        &STATUS_INFO
    }

    async fn filter(&self, dis: &Dispatch, _ctx: &Context, _orig: &Message, name: String) -> crate::error::Result<String> {
        let _ = dis.command_module(&name)?;
        self.command_counter.fetch_add(1, Ordering::Relaxed);
        Ok(name)
    }

    async fn process(&self, dis: &Dispatch, ctx: &Context, orig: &Message, _: Vec<String>) -> crate::error::Result<()> {
        let mut elapsed = START_TIME.elapsed();
        elapsed -= Duration::from_nanos(elapsed.subsec_nanos() as u64);
        let pretty_elapsed = humantime::format_duration(elapsed);
        let sys = systemstat::System::new();
        let load = sys.load_average()?;
        let mem = sys.memory()?;
        let mut sys_uptime = sys.uptime()?;
        sys_uptime -= Duration::from_nanos(sys_uptime.subsec_nanos() as u64);
        let pretty_sys_uptime = humantime::format_duration(sys_uptime);

        let used_mem_mib = (mem.total.0 - mem.free.0) / BYTES_IN_MIB;
        let total_mem_mib = mem.total.0 / BYTES_IN_MIB;

        let shard_man = {
            ctx.data.read().await.get::<ShardManKey>().expect("missing shard manager somehow").clone()
        };

        let shard = ctx.shard_id as usize;
        let total_shards = shard_man.lock()
            .await
            .shards_instantiated()
            .await
            .len();

        let commands_seen = self.command_counter.load(Ordering::Relaxed);
        let stats = crate::db::CONFIG_CACHE.statistics();

        orig.channel_id.send_message(ctx, |e| {
            e.embed(|emb| {
                emb
                .color(GLIM_COLOR)
                    .title("Bot Status")
                    .url(REPO_URL)
                    .field("CPU Load", format!("{:5.2} {:5.2} {:5.2}", load.one, load.five, load.fifteen), true)
                    .field("Memory Usage", format!("{:5} / {:5} MiB", used_mem_mib, total_mem_mib), true)
                    .field("Cache Statistics (misses/accesses)", format!("{} / {}", stats.misses, stats.accesses), true)
                    .field("Uptime", pretty_elapsed, false)
                    .field("Sys Uptime", pretty_sys_uptime, false)
                    .field("Shard Id", shard, true)
                    .field("Shard Count", total_shards, true)
                    .field("Commands Seen", commands_seen, true)
            }).reference_message(orig)
        })
            .await
            .map_err(|e| crate::error::Error::from_err(e, false))?;

        Ok(())
    }
}