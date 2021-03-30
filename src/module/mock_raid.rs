//! Module to mock a raid in a server; for dev purposes.

use crate::module::{Module, UnimplementedModule, ModInfo, Sensitivity};
use serenity::client::Context;
use serenity::model::channel::Message;

use crate::dispatch::{Dispatch, ArcDispatch};
use once_cell::sync::Lazy;
use std::sync::{Weak, Arc};
use arc_swap::{ArcSwap, ArcSwapOption, ArcSwapWeak};
use crate::util::ClapExt;
use crate::util::constraints::ConstrainedU64;
use crate::error::{GuildNotInCache, DeputyConfused};
use serenity::model::prelude::UserId;
use regex::Regex;
use serenity::model::guild::{Guild, PartialMember};
use rand::{thread_rng, Rng};
use rand::prelude::IteratorRandom;
use serenity::builder::CreateMessage;
use futures::StreamExt;
use tracing::Instrument;
use serenity::prelude::EventHandler;
use num::ToPrimitive;
use tracing::field::DisplayValue;
use itertools::Itertools;

#[derive(Default)]
pub struct MockRaidModule {}

#[derive(Debug, structopt::StructOpt)]
#[structopt(name = "mock-raid", about = "Performs a mock raid on the Glimbot backend.")]
struct MockRaidOpt {
    /// The number of messages to be send during the raid.
    #[structopt(default_value = "65536")]
    size: ConstrainedU64<1, {1024 * 1024}>,
    /// The number of threads to use for the raid.
    #[structopt(default_value = "4")]
    threads: ConstrainedU64<1, 64>,
    /// If set, will actually do the raid.
    #[structopt(short, long)]
    start: bool
}

impl MockRaidModule {
}

static CORPUS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    crate::about::LICENSE_HEADER.split_inclusive(char::is_whitespace).collect()
});

struct MockMessageContext {
    members: Vec<UserId>,
    guild: Guild,
    model: Message,
}

impl MockMessageContext {
    pub fn new(g: &Guild, model: &Message) -> Self {

        let mut m = model.clone();
        m.content.clear();
        m.mention_everyone = false;
        m.mention_roles.clear();
        m.mentions.clear();
        m.attachments.clear();
        m.member.take();
        m.mention_channels.clear();

        Self {
            guild: g.clone(),
            members: g.members.keys().cloned().collect(),
            model: m
        }
    }

    fn gen_message_content() -> String {
        let mut o = String::new();
        let mut rng = thread_rng();
        let rand_len = rng.gen_range(1..1500usize);
        while o.len() < rand_len {
            o.push_str(<&str>::clone(CORPUS.iter().choose(&mut rng).unwrap()));
        }

        o
    }

    pub fn gen_message(&self) -> Message {
        let mut rng = thread_rng();
        let author = self.members.iter().choose(&mut rng).unwrap();
        let mut msg = self.model.clone();
        let mem = self.guild.members.get(author).unwrap();
        msg.author = mem.user.clone();
        msg.member = serde_json::to_value(mem).and_then(serde_json::from_value).ok();
        msg.content = Self::gen_message_content();

        msg
    }
}

impl_err!(DispatchMissing, "Dispatch wasn't set.", true);

#[async_trait::async_trait]
impl Module for MockRaidModule {
    fn info(&self) -> &ModInfo {
        static INFO: Lazy<ModInfo> = Lazy::new(|| {
            ModInfo::with_name("mock-raid", "mocks a raid in this server in glimbot.")
                .with_sensitivity(Sensitivity::Owner)
                .with_command(true)
        });
        &INFO
    }

    async fn process(&self, dis: &Dispatch, ctx: &Context, orig: &Message, command: Vec<String>) -> crate::error::Result<()> {
        let g = orig.guild(ctx).await.ok_or(GuildNotInCache)?;
        // This should only be run in a guild the bot owner owns.
        if orig.author.id != g.owner_id {
            return Err(DeputyConfused.into());
        }
        let opts = MockRaidOpt::from_iter_with_help(command)?;

        if !opts.start {
            info!("would have started the raid with {:?}, but start was not started.", opts);
            return Ok(());
        }

        let mmc = MockMessageContext::new(&g, orig);

        info!("pregenerating messages...");
        let gen = (0..opts.size.to_usize().unwrap()).map(|i| (i, mmc.gen_message())).collect_vec();
        info!("done");

        let start = std::time::Instant::now();
        futures::stream::iter(gen)
            .for_each_concurrent(opts.threads.to_usize().unwrap(), |(i, m)| {
                dis.message(ctx.clone(), m)
                    .instrument(info_span!("mock raid message", idx = i))

            })
            .await;
        let e = start.elapsed();
        info!("raid used {thd_cnt} threads and processed {msg_cnt} messages over {display:?}, rate was {rate} msg/s",
            thd_cnt = opts.threads,
            msg_cnt = opts.size,
            display = e,
            rate = opts.size.to_f64().unwrap() / e.as_secs_f64()
        );

        Ok(())

    }
}