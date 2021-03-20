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
use arc_swap::{ArcSwap, RefCnt};

pub type CacheValue<V, Tag> = Arc<arc_swap::ArcSwapOption<(Tag, V)>>;

#[derive(Debug)]
pub struct Cached<V, Tag>(Arc<(Tag, V)>);

impl<V, Tag> Clone for Cached<V, Tag> {
    fn clone(&self) -> Self {
        Cached(self.0.clone())
    }
}

impl<V, Tag> Deref for Cached<V, Tag> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.0.1
    }
}

impl<V, Tag> AsRef<V> for Cached<V, Tag> {
    fn as_ref(&self) -> &V {
        &self.0.1
    }
}

impl<V, Tag> Borrow<V> for Cached<V, Tag> {
    fn borrow(&self) -> &V {
        self.as_ref()
    }
}


impl<V, Tag> Cached<V, Tag> {
    pub fn tag(&self) -> &Tag {
        &self.0.0
    }
}

pub trait EvictionStrategy: Sized + fmt::Debug {
    type Tag: fmt::Debug + Sized + Clone + Send + Sync;
    fn should_evict(&self, t: &Self::Tag) -> bool;
    fn create_tag(&self, g: GuildId) -> Self::Tag;
}

#[derive(Copy, Clone, Debug)]
pub struct TimedEvictionStrategy {
    ttl: std::time::Duration
}

impl TimedEvictionStrategy {
    pub fn new(ttl: std::time::Duration) -> Self {
        TimedEvictionStrategy { ttl }
    }
}

impl EvictionStrategy for TimedEvictionStrategy {
    type Tag = std::time::Instant;

    fn should_evict(&self, t: &Self::Tag) -> bool {
        self.ttl > t.elapsed()
    }

    fn create_tag(&self, _g: GuildId) -> Self::Tag {
        Instant::now()
    }
}

#[derive(Copy, Clone, Debug)]
pub struct NullEvictionStrategy;
impl EvictionStrategy for NullEvictionStrategy {
    type Tag = ();

    fn should_evict(&self, _t: &Self::Tag) -> bool {
        false
    }

    fn create_tag(&self, _g: GuildId) -> Self::Tag {}
}


#[derive(Debug)]
pub struct Cache<V: Send + Sync, S: EvictionStrategy + Send + Sync = NullEvictionStrategy> {
    cache: ArcSwap<im::HashMap<GuildId, CacheValue<V, S::Tag>>>,
    strategy: S,
}

impl<V: Send + Sync, S: EvictionStrategy + Send + Sync> Cache<V, S> {
    pub fn new(strategy: S) -> Self {
        Self {
            cache: Default::default(),
            strategy,
        }
    }

    pub fn ensure_entry<'a>(&'a self, g: GuildId) -> impl Access<CacheValue<V, S::Tag>, Guard = impl Send + Deref<Target=CacheValue<V, S::Tag>>> + Send + 'a {
        if self.cache.load().get(&g).is_none() {
            self.cache.rcu(|c| {
                let mut c = im::HashMap::clone(c);
                if !c.contains_key(&g) {
                    c.insert(g, CacheValue::default());
                }
                c
            });
        }

        Map::new(&self.cache, move |c: &im::HashMap<GuildId, CacheValue<V, S::Tag>>| c.get(&g).unwrap())
    }

    pub async fn get_or_insert_with<Fut>(&self, g: GuildId, f: Fut) -> crate::error::Result<Cached<V, S::Tag>>
        where Fut: Future<Output=crate::error::Result<V>> {
        let cache = self.ensure_entry(g).load();
        let c: &CacheValue<V, S::Tag> = cache.deref();
        let needs_reset = c.load().as_ref().map(|a| self.strategy.should_evict(&(*a).0))
            .unwrap_or(true);

        let out = if needs_reset {
            let v = f.await?;
            let ins = Arc::new((self.strategy.create_tag(g), v));
            c.store(Some(ins.clone()));
            ins
        } else {
            c.load_full().unwrap()
        };

        Ok(Cached(out))
    }

    pub fn insert(&self, g: GuildId, v: V) {
        self.ensure_entry(g).load().deref().store(Some(Arc::new((self.strategy.create_tag(g), v))));
    }

    pub fn get(&self, g: GuildId) -> Option<Cached<V, S::Tag>> {
        self.ensure_entry(g).load().deref().load_full().map(Cached)
    }
}

#[derive(Debug)]
pub struct TimedCache<V: Send + Sync> {
    inner: Cache<V, TimedEvictionStrategy>
}

impl<V: Send + Sync> AsRef<Cache<V, TimedEvictionStrategy>> for TimedCache<V> {
    fn as_ref(&self) -> &Cache<V, TimedEvictionStrategy> {
        &self.inner
    }
}

impl<V: Send + Sync> Deref for TimedCache<V> {
    type Target = Cache<V, TimedEvictionStrategy>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<V: Send + Sync> Borrow<Cache<V, TimedEvictionStrategy>> for TimedCache<V> {
    fn borrow(&self) -> &Cache<V, TimedEvictionStrategy> {
        &self.inner
    }
}

impl<V: Send + Sync> TimedCache<V> {
    pub fn new(ttl: std::time::Duration) -> Self {
        Self {
            inner: Cache::new(TimedEvictionStrategy::new(ttl))
        }
    }
}