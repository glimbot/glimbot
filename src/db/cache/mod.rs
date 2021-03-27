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
use arc_swap::{ArcSwap, RefCnt, AsRaw, Guard};
use std::hash::Hash;

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

pub trait EvictionStrategy<K>: Sized + fmt::Debug where K: Send + Sync + Hash + Eq + Clone {
    type Tag: fmt::Debug + Sized + Clone + Send + Sync;
    fn should_evict(&self, t: &Self::Tag) -> bool;
    fn create_tag(&self, k: &K) -> Self::Tag;
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

impl<K: Send + Sync + Hash + Eq + Clone> EvictionStrategy<K> for TimedEvictionStrategy {
    type Tag = std::time::Instant;

    fn should_evict(&self, t: &Self::Tag) -> bool {
        self.ttl < t.elapsed()
    }

    fn create_tag(&self, _g: &K) -> Self::Tag {
        Instant::now()
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct NullEvictionStrategy;
impl<K: Send + Sync + Hash + Eq + Clone> EvictionStrategy<K> for NullEvictionStrategy {
    type Tag = ();

    fn should_evict(&self, _t: &Self::Tag) -> bool {
        false
    }

    fn create_tag(&self, _g: &K) -> Self::Tag {}
}


#[derive(Debug)]
pub struct Cache<K: Send + Sync + Hash + Eq + Clone, V: Send + Sync, S: EvictionStrategy<K> + Send + Sync = NullEvictionStrategy> {
    cache: ArcSwap<im::HashMap<K, CacheValue<V, S::Tag>>>,
    strategy: S,

}

impl<K: Send + Sync + Hash + Eq + Clone, V: Send + Sync, S: EvictionStrategy<K> + Send + Sync> Cache<K, V, S> {
    pub fn new(strategy: S) -> Self {
        Self {
            cache: Default::default(),
            strategy,
        }
    }

    pub fn ensure_entry<'a>(&'a self, k: &K) -> impl Access<CacheValue<V, S::Tag>, Guard = impl Send + Deref<Target=CacheValue<V, S::Tag>>> + Send + 'a {
        if self.cache.load().get(k).is_none() {
            self.cache.rcu(|c| {
                let mut c = im::HashMap::clone(c);
                if !c.contains_key(k) {
                    c.insert(k.clone(), CacheValue::default());
                }
                c
            });
        }
        let k = k.clone();

        Map::new(&self.cache, move |c: &im::HashMap<K, CacheValue<V, S::Tag>>| c.get(&k).unwrap())
    }

    /// This is very subtly wrong
    pub async fn get_or_insert_with<Fut>(&self, key: &K, f: Fut) -> crate::error::Result<Cached<V, S::Tag>>
        where Fut: Future<Output=crate::error::Result<V>> {
        let cache = self.ensure_entry(key).load();
        let c: &CacheValue<V, S::Tag> = cache.deref();
        let cloaded = c.load_full();

        let needs_reset = cloaded.as_ref().map(|a| self.strategy.should_evict(&(*a).0))
            .unwrap_or(true);

        let out = if needs_reset {
            let v = f.await?;
            let ins = Arc::new((self.strategy.create_tag(key), v));
            let mut out = ins.clone();
            c.rcu(|r| {
                if let Some(r) = r {
                    out = r.clone();
                    Some(r.clone())
                } else {
                    out = ins.clone();
                    Some(ins.clone())
                }
            });
            out
        } else {
            // The only way to get here is if `needs_reset` is false, which means
            // the option was full.
            cloaded.unwrap()
        };

        Ok(Cached(out))
    }

    pub fn insert(&self, key: &K, v: V) {
        self.ensure_entry(key).load().deref().store(Some(Arc::new((self.strategy.create_tag(key), v))));
    }

    pub fn get(&self, key: &K) -> Option<Cached<V, S::Tag>> {
        let cache = self.ensure_entry(&key).load();
        let c: &CacheValue<V, S::Tag> = cache.deref();

        let mut res = None;
        c.rcu(|f| {
            let needs_reset = f.as_ref().map(|a| self.strategy.should_evict(&(*a).0))
                .unwrap_or(true);
            if needs_reset {
                res = None;
                None
            } else {
                res = f.clone();
                f.clone()
            }
        });

        res.map(Cached)
    }

    pub fn get_or_insert_sync(&self, key: &K, val: impl FnOnce() -> V) -> Cached<V, S::Tag> {
        futures::executor::block_on(self.get_or_insert_with(key, async { Ok(val()) })).unwrap()
    }

    pub fn get_or_insert_default(&self, key: &K) -> Cached<V, S::Tag> where V: Default {
        self.get_or_insert_sync(key, V::default)
    }

    pub fn reset(&self, key: &K) where V: Default {
        self.insert(key, V::default())
    }

    pub fn remove(&self, key: &K) -> Option<Cached<V, S::Tag>> {
        let mut out = None;
        self.cache.rcu(|r| {
            if r.contains_key(key) {
                let mut o = im::HashMap::clone(r);
                out = o.remove(key);
                Arc::new(o)
            } else {
                r.clone()
            }
        });
        out.and_then(|cv| cv.load_full()).map(Cached)
    }

    pub fn update(&self, key: &K, update_fn: impl Fn(Option<&V>) -> Option<V>) -> Update<V, S::Tag> {
        let cache = self.ensure_entry(&key).load();
        let c: &CacheValue<V, S::Tag> = cache.deref();

        let mut out = None;
        c.rcu(|o| {
            let needs_reset = o.as_ref().map(|a| self.strategy.should_evict(&(*a).0))
                .unwrap_or(true);
            let pass_val = if needs_reset {
                None
            } else {
                o.clone()
            };
            let new = update_fn(pass_val.as_ref().map(|c| &c.1));
            let new = new.map(|v| Arc::new((self.strategy.create_tag(key), v)));
            out = Some(Update {
                old: pass_val.map(Cached),
                new: new.clone().map(Cached),
            });
            new
        });
        out.unwrap()
    }

    pub fn update_and_fetch(&self, key: &K, update_fn: impl Fn(Option<&V>) -> Option<V>) -> Option<Cached<V, S::Tag>> {
        self.update(key, update_fn).new
    }

    pub fn fetch_and_update(&self, key: &K, update_fn: impl Fn(Option<&V>) -> Option<V>) -> Option<Cached<V, S::Tag>> {
        self.update(key, update_fn).old
    }

}

#[derive(Debug)]
pub struct Update<V, Tag> {
    pub old: Option<Cached<V, Tag>>,
    pub new: Option<Cached<V, Tag>>
}

impl<K: Send + Sync + Hash + Eq + Clone, V: Send + Sync> Cache<K, V, NullEvictionStrategy> {
    pub fn null() -> Self {
        Cache::new(NullEvictionStrategy)
    }
}

impl<K: Send + Sync + Hash + Eq + Clone, V: Send + Sync, S: EvictionStrategy<K> + Send + Sync + Default> Default for Cache<K, V, S> {
    fn default() -> Self {
        Self::new(S::default())
    }
}

#[derive(Debug)]
pub struct TimedCache<K: Send + Sync + Hash + Eq + Clone, V: Send + Sync> {
    inner: Cache<K, V, TimedEvictionStrategy>
}

impl<K: Send + Sync + Hash + Eq + Clone, V: Send + Sync> AsRef<Cache<K, V, TimedEvictionStrategy>> for TimedCache<K, V> {
    fn as_ref(&self) -> &Cache<K, V, TimedEvictionStrategy> {
        &self.inner
    }
}

impl<K: Send + Sync + Hash + Eq + Clone, V: Send + Sync> Deref for TimedCache<K, V> {
    type Target = Cache<K, V, TimedEvictionStrategy>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<K: Send + Sync + Hash + Eq + Clone, V: Send + Sync> Borrow<Cache<K, V, TimedEvictionStrategy>> for TimedCache<K, V> {
    fn borrow(&self) -> &Cache<K, V, TimedEvictionStrategy> {
        &self.inner
    }
}

impl<K: Send + Sync + Hash + Eq + Clone, V: Send + Sync> TimedCache<K, V> {
    pub fn new(ttl: std::time::Duration) -> Self {
        Self {
            inner: Cache::new(TimedEvictionStrategy::new(ttl))
        }
    }
}