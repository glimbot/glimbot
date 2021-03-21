use once_cell::sync::Lazy;
use std::borrow::Cow;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::marker::PhantomData;
use std::future::Future;
use std::sync::Arc;
use sled::{IVec, Subscriber};
use sled::transaction as trans;
use sled::transaction::{TransactionResult, ConflictableTransactionError, TransactionError};
use sled::transaction::ConflictableTransactionError::Abort;
use crate::error::Error;
use futures::Stream;
use std::convert::TryFrom;
use zerocopy::{U128, AsBytes, LayoutVerified, U64};
use byteorder::BigEndian;
use num::{FromPrimitive, ToPrimitive};

static DB: Lazy<sled::Db> = Lazy::new(|| {
    sled::Config::default()
        // using temporary keeps the file in RAM, which keeps it nice and async friendly.
        .temporary(true)
        .mode(sled::Mode::HighThroughput)
        .open()
        .expect("failed while opening cache")
});

impl_err!(BadLayout, "Improper layout seen for type.", false);

pub trait CacheKey: Sized + Clone {
    fn to_cache_key(&self) -> Cow<[u8]>;
    fn from_bytes(v: &[u8]) -> crate::error::Result<Self>;
}

pub trait Prefix {
    const PREFIX: &'static [u8];
}

impl<T: zerocopy::FromBytes + zerocopy::AsBytes + zerocopy::Unaligned + Clone> CacheKey for T {
    fn to_cache_key(&self) -> Cow<[u8]> {
        Cow::Borrowed(self.as_bytes())
    }

    fn from_bytes(v: &[u8]) -> crate::error::Result<Self> {
        let l = LayoutVerified::<_, T>::new(v.as_ref()).ok_or(BadLayout)?;
        let o_ref: &T = l.into_ref();
        Ok(o_ref.clone())
    }
}

pub trait CacheValue: Serialize + DeserializeOwned {}

impl<T> CacheValue for T where T: Serialize + DeserializeOwned {}

#[derive(Clone)]
pub struct CacheView<P: Prefix, K: CacheKey, V: CacheValue> {
    tree: sled::Tree,
    _phantom: PhantomData<fn() -> (P, K, V)>
}

impl<P: Prefix, K, V> CacheView<P, K, V> where K: CacheKey, V: CacheValue {
    pub fn new() -> crate::error::Result<Self> {
        let tree = DB.open_tree(P::PREFIX)?;
        Ok(Self { tree, _phantom: Default::default()})
    }

    pub fn get(&self, k: &K) -> crate::error::Result<Option<V>> {
        let v = self.tree.get(k.to_cache_key())?;
        Ok(v.map(|v| rmp_serde::from_read_ref(&v)).transpose()?)
    }

    pub fn update_and_fetch(&self, k: &K, f: impl Fn(Option<&V>) -> crate::error::Result<Option<V>>) -> crate::error::Result<Option<V>> {
        let k = k.to_cache_key();
        let res: TransactionResult<Option<V>, crate::error::Error> = self.tree.transaction(
            |t| {
                let v = t.get(&k).map_err(ConflictableTransactionError::from)?;
                let val = v.as_ref().map(rmp_serde::from_read_ref).transpose().map_err(|e| Abort(e.into()))?;
                let ov = f(val.as_ref()).map_err(Abort)?;
                let v = ov.as_ref().map(|v| rmp_serde::to_vec(&v)).transpose().map_err(|e| Abort(e.into()))?;
                if let Some(v) = v {
                    t.insert(k.as_ref(), v)?;
                } else {
                    t.remove(k.as_ref())?;
                }
                Ok(ov)
            }
        );

        res.map_err(|e| match e {
            TransactionError::Abort(e) => {e}
            TransactionError::Storage(e) => {e.into()}
        })
    }

    pub fn fetch_and_update(&self, k: &K, f: impl Fn(Option<&V>) -> crate::error::Result<Option<V>>) -> crate::error::Result<Option<V>> {
        let k = k.to_cache_key();
        let res: TransactionResult<Option<V>, crate::error::Error> = self.tree.transaction(
            |t| {
                let v = t.get(&k).map_err(ConflictableTransactionError::from)?;
                let val = v.as_ref().map(rmp_serde::from_read_ref).transpose().map_err(|e| Abort(e.into()))?;
                let ov = f(val.as_ref()).map_err(Abort)?;
                let v = ov.as_ref().map(|v| rmp_serde::to_vec(&v)).transpose().map_err(|e| Abort(e.into()))?;
                if let Some(v) = v {
                    t.insert(k.as_ref(), v)?;
                } else {
                    t.remove(k.as_ref())?;
                }
                Ok(val)
            }
        );

        res.map_err(|e| match e {
            TransactionError::Abort(e) => {e}
            TransactionError::Storage(e) => {e.into()}
        })
    }

    pub fn watch(&self, k: impl CacheKey) -> impl Stream<Item=crate::error::Result<Event<K, V>>> {
        let subscriber = self.tree.watch_prefix(k.to_cache_key());
        futures::stream::try_unfold(subscriber, move |mut s| {
            async move {
                let data = (&mut s).await;
                let o = data
                    .map(Event::try_from)
                    .transpose()?;
                Ok(o.map(|o| (o, s)))
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Event<K: CacheKey, V: CacheValue> {
    Insert {
        key: K,
        value: V
    },
    Remove {
        key: K
    }
}



impl <K: CacheKey, V: CacheValue> TryFrom<sled::Event> for Event<K, V> {
    type Error = crate::error::Error;

    fn try_from(e: sled::Event) -> Result<Self, Self::Error> {
        match e {
            sled::Event::Insert { key, value } => {
                Ok(Event::Insert { key: K::from_bytes(key.as_ref())?, value: rmp_serde::from_read_ref(&value)? })
            }
            sled::Event::Remove { key } => { Ok(Self::Remove { key: K::from_bytes(key.as_ref())? }) }
        }
    }
}

impl<K: CacheKey, V: CacheValue> Event<K, V> {
    pub fn key(&self) -> &K {
        match self {
            Event::Insert { key, .. } => {&key}
            Event::Remove { key } => {&key}
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, zerocopy::FromBytes, zerocopy::AsBytes, zerocopy::Unaligned)]
#[repr(transparent)]
pub struct IntegerKey {
    val: U64<BigEndian>
}

impl IntegerKey {
    pub fn new(i: impl Into<u64>) -> Self {
        IntegerKey {
            val: U64::new(i.into())
        }
    }
}

impl FromPrimitive for IntegerKey {
    fn from_i64(n: i64) -> Option<Self> {
        Some(Self::new(n as u64))
    }

    fn from_u64(n: u64) -> Option<Self> {
        Some(Self::new(n))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, shrinkwraprs::Shrinkwrap)]
pub struct IdKey<Id: From<u64> + Into<u64>> {
    #[shrinkwrap(main_field)] inner: IntegerKey,
    _id: PhantomData<fn() -> Id>
}

impl<Id: From<u64> + Into<u64>> From<Id> for IdKey<Id> {
    fn from(e: Id) -> Self {
        Self {
            inner: IntegerKey::new(e),
            _id: Default::default()
        }
    }
}

impl<Id: From<u64> + Into<u64>> IdKey<Id> {
    pub fn into_inner(self) -> Id {
        Id::from(self.inner.val.get())
    }
}

impl<Id: From<u64> + Into<u64>> From<IdKey<Id>> for IntegerKey {
    fn from(e: IdKey<Id>) -> Self {
        e.inner
    }
}

impl<Id: From<u64> + Into<u64> + Clone> CacheKey for IdKey<Id> {
    fn to_cache_key(&self) -> Cow<[u8]> {
        self.inner.to_cache_key()
    }

    fn from_bytes(v: &[u8]) -> crate::error::Result<Self> {
        Ok(Self {
            inner: IntegerKey::from_bytes(v)?,
            _id: Default::default(),
        })
    }
}

#[macro_export]
macro_rules! count_tts {
    () => (0);
    ($one:tt) => (1);
    ($($a:tt $b:tt)+) => (count_tts!($($a)+) << 1);
    ($odd:tt $($a:tt $b:tt)+) => (count_tts!($($a)+) << 1 | 1);
}

pub trait FromTuple {
    type Dest;
    fn into_array(self) -> Self::Dest;
    fn from_array(arr: Self::Dest) -> Self;
}

impl<A, B> FromTuple for (A, B) where A: From<u64> + Into<u64> + Clone, B: From<u64> + Into<u64> + Clone  {
    type Dest = [IntegerKey; 2];

    fn into_array(self) -> Self::Dest {
        let (a, b) = self;
        [IdKey::from(a).into(), IdKey::from(b).into()]
    }

    fn from_array(arr: Self::Dest) -> Self {
        let [a, b] = arr;
        (A::from(a.val.get()), B::from(b.val.get()))
    }
}

impl<A, B, C> FromTuple for (A, B, C) where A: From<u64> + Into<u64> + Clone, B: From<u64> + Into<u64> + Clone, C: From<u64> + Into<u64> + Clone  {
    type Dest = [IntegerKey; 3];

    fn into_array(self) -> Self::Dest {
        let (a, b, c) = self;
        [IdKey::from(a).into(), IdKey::from(b).into(), IdKey::from(c).into()]
    }

    fn from_array(arr: Self::Dest) -> Self {
        let [a, b, c] = arr;
        (A::from(a.val.get()), B::from(b.val.get()), C::from(c.val.get()))
    }
}

#[macro_export]
macro_rules! impl_id_key {
    ($name:ident, $($ids:path),+) => {

    #[derive(Debug, Copy, Clone, Eq, PartialEq, zerocopy::FromBytes, zerocopy::AsBytes, zerocopy::Unaligned)]
    #[repr(C)]
    pub struct $name {
        inner: [$crate::db::cache::kv::IntegerKey; count_tts!($($ids)+)]
    }

    impl $name {
        pub fn new(orig: ($($ids,)+)) -> Self {
        use $crate::db::cache::kv::FromTuple;
            Self {
                inner: orig.into_array()
            }
        }

        pub fn into_inner(self) -> ($($ids,)+) {
            $crate::db::cache::kv::FromTuple::from_array(self.inner)
        }
    }

    };
}

#[macro_export]
macro_rules! impl_prefix {
    ($name:ident) => {

    pub struct $name;
    impl $crate::db::cache::kv::Prefix for $name {
        const PREFIX: &'static [u8] = stringify!($name).as_bytes();
    }

    };
}