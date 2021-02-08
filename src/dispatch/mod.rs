use serenity::model::id::UserId;
use once_cell::sync::OnceCell;
use tokio::sync::RwLock;
use serenity::client::{Context, EventHandler};
use serenity::model::gateway::{Ready, Activity};
use serenity::model::channel::Message;

pub struct Dispatch {
    owner: UserId,
}

impl Dispatch {
    pub fn new(owner: UserId) -> Self {
        Self {
            owner,
        }
    }
}

#[async_trait::async_trait]
impl EventHandler for Dispatch {
    async fn message(&self, _ctx: Context, _new_message: Message) {
        unimplemented!()
    }

    async fn ready(&self, ctx: Context, rdy: Ready) {
        info!("up and running in {} guilds.", rdy.guilds.len());
        ctx.set_activity(Activity::playing("Cultist Simulator")).await;
    }
}