//! Contains implementation of caching for per guild objects.

use std::fmt;
use dashmap::DashMap;
use serenity::model::id::GuildId;
use std::sync::Arc;
use std::future::Future;
use std::process::Output;
use std::time::Instant;
use arc_swap::access::{Access, Map};
use std::ops::Deref;
use std::borrow::Borrow;
use arc_swap::{ArcSwap, Cache, RefCnt};

pub type CacheValue<V> = Arc<arc_swap::ArcSwapOption<(std::time::Instant, V)>>;

#[derive(Debug)]
pub struct Cached<V>(Arc<(std::time::Instant, V)>);

impl<V> Clone for Cached<V> {
    fn clone(&self) -> Self {
        Cached(self.0.clone())
    }
}

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
pub struct TimedCache<V: Send + Sync> {
    cache: ArcSwap<im::HashMap<GuildId, CacheValue<V>>>,
    ttl: std::time::Duration,
}

impl<V: Send + Sync> TimedCache<V> {
    pub fn new(ttl: std::time::Duration) -> Self {
        Self {
            cache: Default::default(),
            ttl,
        }
    }

    pub fn ensure_entry<'a>(&'a self, g: GuildId) -> impl Access<CacheValue<V>, Guard = impl Send + Deref<Target=CacheValue<V>>> + Send + 'a {
        if let None = self.cache.load().get(&g) {
            self.cache.rcu(|c| {
                let mut c = im::HashMap::clone(c);
                if !c.contains_key(&g) {
                    c.insert(g, CacheValue::default());
                }
                c
            });
        }

        Map::new(&self.cache, move |c: &im::HashMap<GuildId, CacheValue<V>>| c.get(&g).unwrap())
    }

    pub async fn get_or_insert_with<Fut>(&self, g: GuildId, f: Fut) -> crate::error::Result<Cached<V>>
        where Fut: Future<Output=crate::error::Result<V>> {
        let cache = self.ensure_entry(g).load();
        let c: &CacheValue<V> = cache.deref();
        let needs_reset = c.load().as_ref().map(|a| (*a).0)
            .map(|i| i.elapsed() > self.ttl)
            .unwrap_or(true);

        let out = if needs_reset {
            let v = f.await?;
            let ins = Arc::new((Instant::now(), v));
            c.store(Some(ins.clone()));
            ins
        } else {
            c.load_full().unwrap()
        };

        Ok(Cached(out))
    }

    pub fn insert(&self, g: GuildId, v: V) {
        self.ensure_entry(g).load().deref().store(Some(Arc::new((Instant::now(), v))));
    }

    pub fn get(&self, g: GuildId) -> Option<Cached<V>> {
        self.ensure_entry(g).load().deref().load_full().map(Cached)
    }
}