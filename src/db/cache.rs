//! Contains implementation of caching for per guild objects.

use std::fmt;
use dashmap::DashMap;
use serenity::model::id::GuildId;
use std::sync::Arc;
use std::future::Future;
use std::process::Output;
use std::time::Instant;
use arc_swap::access::Access;
use std::ops::Deref;
use std::borrow::Borrow;

pub type CacheValue<V> = arc_swap::ArcSwapOption<(std::time::Instant, V)>;

#[derive(Debug, Clone)]
pub struct Cached<V>(Arc<(std::time::Instant, V)>);

impl<V> Deref for Cached<V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.0.1
    }
}

impl<V> AsRef<V> for Cached<V> {
    fn as_ref(&self) -> &V {
        &self.0.1
    }
}

impl<V> Borrow<V> for Cached<V> {
    fn borrow(&self) -> &V {
        self.as_ref()
    }
}

impl<V> Cached<V> {
    pub fn expiry(&self) -> Instant {
        self.0.0
    }
}

#[derive(Debug)]
pub struct TimedCache<V> {
    cache: DashMap<GuildId, CacheValue<V>>,
    ttl: std::time::Duration,
}

impl<V> TimedCache<V> {
    pub fn new(ttl: std::time::Duration) -> Self {
        Self {
            cache: Default::default(),
            ttl
        }
    }

    pub async fn get_or_insert_with<Fut>(&self, g: GuildId, f: Fut) -> crate::error::Result<Cached<V>>
        where Fut: Future<Output=crate::error::Result<V>> {
        let cache = self.cache.entry(g).or_default().value().load();
        let needs_reset = cache.as_ref().map(|a| (*a).0)
            .map(|i| i.elapsed() > self.ttl)
            .unwrap_or(true);

        let out = if needs_reset {
            let v = f.await?;
            let ins = Arc::new((Instant::now(), v));
            self.cache.get(&g).unwrap().store(Some(ins.clone()));
            ins
        } else {
            cache.as_ref().unwrap().clone()
        };

        Ok(Cached(out))
    }

    pub fn insert(&self, g: GuildId, v: V) {
        self.cache.entry(g).or_default().value().store(Some(Arc::new((Instant::now(), v))));
    }

    pub fn get(&self, g: GuildId) -> Option<Cached<V>> {
        self.cache.entry(g).or_default().value().load_full().map(Cached)
    }
}