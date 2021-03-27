#![feature(iter_map_while)]
#![feature(map_first_last)]

#[cfg(target_env = "gnu")]
use jemallocator::Jemalloc;
use parking_lot::{Mutex, RwLock, RwLockUpgradableReadGuard};
use std::collections::BTreeSet;
use std::sync::Arc;
use itertools::Itertools;
use glimbot::util::ordset::OrdSet;
use criterion::{Criterion, BatchSize};
use rayon::prelude::*;
use rand::distributions::{WeightedIndex, Bernoulli, Uniform};
use once_cell::sync::Lazy;
use rand::{Rng, thread_rng};
use rand::distributions::uniform::UniformInt;
use smallvec::SmallVec;
use std::num::NonZeroUsize;


#[doc(hidden)]
#[cfg(target_env = "gnu")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[macro_use]
extern crate criterion;

type StdOrdSet<T> = RwLock<BTreeSet<Arc<T>>>;

trait ConcurrentOrderedSet<T: Ord + Clone + Send + Sync>: Sized + Sync {
    type Inner;
    fn contains(&self, val: &T) -> bool;
    fn insert(&self, val: T) -> bool;
    fn remove(&self, val: &T) -> bool;
    fn insert_all(&self, val: impl Iterator<Item=T>) -> usize;
    fn partition(&self, val: &T) -> (Self::Inner, Self::Inner);

    fn do_op(&self, op: Op<T>) {
        match op {
            Op::Insert(v) => { criterion::black_box(self.insert(v)); }
            Op::InsertAll(vs) => { criterion::black_box(self.insert_all(vs.into_iter())); }
            Op::Remove(v) => { criterion::black_box(self.remove(&v)); }
            Op::Contains(v) => { criterion::black_box(self.contains(&v)); }
            Op::Partition(v) => { criterion::black_box(self.partition(&v)); }
        };
    }
}

const CAPACITY: usize = 1024;

impl<T> ConcurrentOrderedSet<T> for StdOrdSet<T> where T: Ord + Clone + Send + Sync {
    type Inner = BTreeSet<Arc<T>>;

    fn contains(&self, val: &T) -> bool {
        let rg = self.read().contains(val);
        rg
    }

    fn insert(&self, val: T) -> bool {
        let mut wg = self.write();
        let o = wg.insert(Arc::new(val));
        while wg.len() > CAPACITY {
            wg.pop_first();
        }
        o
    }

    fn remove(&self, val: &T) -> bool {
        let ug = self.upgradable_read();
        if ug.contains(val) {
            let mut wg = RwLockUpgradableReadGuard::upgrade(ug);
            wg.remove(val)
        } else {
            false
        }
    }

    fn insert_all(&self, val: impl Iterator<Item=T>) -> usize {
        let mut wg = self.write();
        let o = val.filter_map(|v|
            if wg.insert(Arc::new(v)) {
                Some(())
            } else {
                None
            }
        ).count();
        while wg.len() > CAPACITY {
            wg.pop_first();
        }
        o
    }

    fn partition(&self, val: &T) -> (Self::Inner, Self::Inner) {
        let mut inner = self.read().clone();
        let right = inner.split_off(val);
        (inner, right)
    }
}

impl<T> ConcurrentOrderedSet<T> for OrdSet<T> where T: Ord + Clone + Send + Sync {
    type Inner = im::Vector<T>;

    fn contains(&self, val: &T) -> bool {
        OrdSet::contains(self, val)
    }

    fn insert(&self, val: T) -> bool {
        OrdSet::insert(self, val)
    }

    fn remove(&self, val: &T) -> bool {
        OrdSet::remove(self, val)
    }

    fn insert_all(&self, val: impl Iterator<Item=T>) -> usize {
        OrdSet::insert_all(self, val)
    }

    fn partition(&self, val: &T) -> (Self::Inner, Self::Inner) {
        self.partitioned(val)
    }
}

#[derive(Debug, Clone)]
enum Op<T> {
    Insert(T),
    InsertAll(SmallVec<[T; 16]>),
    Remove(T),
    Contains(T),
    Partition(T),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum OpKind {
    Insert,
    InsertAll,
    Remove,
    Contains,
    Partition,
}

impl OpKind {
    pub fn make_op<T, I>(self, data: &mut I) -> Option<Op<T>> where I: Iterator<Item=T> {
        let out = match self {
            OpKind::Insert => {
                Op::Insert(data.next()?)
            }
            OpKind::InsertAll => {
                Op::InsertAll(data.take(16).collect())
            }
            OpKind::Remove => {
                Op::Remove(data.next()?)
            }
            OpKind::Contains => {
                Op::Contains(data.next()?)
            }
            OpKind::Partition => {
                Op::Partition(data.next()?)
            }
        };
        Some(out)
    }
}

struct OpGenerator {
    weights: WeightedIndex<usize>,
}

impl OpGenerator {
    pub fn new() -> Self {
        Self {
weights: WeightedIndex::new(
    &[
        70usize, // Insert
        2, // Insert All
        20, // Remove
        3, // Contains
        20 // Partition
    ]
).unwrap()
        }
    }

    pub fn select_kind(&self) -> OpKind {
        let idx = thread_rng().sample(&self.weights);
        Self::map_kind(idx)
    }

    fn map_kind(i: usize) -> OpKind {
        use OpKind::*;
        const MAP: [OpKind; 5] = [Insert, InsertAll, Remove, Contains, Partition];
        MAP[i]
    }
}

fn generate_ops() -> impl Iterator<Item=Op<usize>> {
    let jitter = Uniform::new_inclusive(0, 5usize);
    let og = OpGenerator::new();

    let mut rng = thread_rng();

    let mut i = (0..).map(move |i| i + rng.sample(&jitter));
    std::iter::repeat_with(move || og.select_kind())
        .map_while(move |k| k.make_op(&mut i))
}

fn do_ops<C, T>(m: &C, o: Vec<Op<T>>) where C: ConcurrentOrderedSet<T>, T: Ord + Clone + Send + Sync {
    Vec::into_par_iter(o)
        .for_each(|o: Op<_>| {
            m.do_op(o);
        });
}

const BATCH_SIZE: usize = 1024;

fn bench_ops(c: &mut Criterion) {
    let mut o = generate_ops();
    c.bench_function(
        "btreeset",
        |b| {
            let set = StdOrdSet::default();
            b.iter_batched(
                || o.by_ref().take(BATCH_SIZE).collect_vec(),
                |i| {
                    do_ops(&set, i)
                },
                BatchSize::SmallInput
            )
        });

    c.bench_function(
        "handroll",
        |b| {
            let m = OrdSet::new(NonZeroUsize::new(CAPACITY));
            b.iter_batched(
                || o.by_ref().take(BATCH_SIZE).collect_vec(),
                |i| {
                    do_ops(&m, i)
                },
                BatchSize::SmallInput
            )
        });
}

criterion_group!(ordset, bench_ops);

criterion_main!(ordset);