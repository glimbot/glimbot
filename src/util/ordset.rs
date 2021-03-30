use std::fmt;
use std::fmt::Formatter;
use parking_lot::RwLock;
use parking_lot::lock_api::RwLockUpgradableReadGuard;
use crate::util::CoalesceResultExt;
use itertools::Itertools;
use std::borrow::Borrow;
use std::num::NonZeroUsize;
use std::iter::FromIterator;
use arc_swap::ArcSwap;
use std::sync::Arc;
use arc_swap::access::Access;
use std::ops::Deref;

#[derive(shrinkwraprs::Shrinkwrap, Clone)]
#[shrinkwrap(mutable)]
struct FixedSizedInner<T> {
    #[shrinkwrap(main_field)] inner: im::Vector<T>,
    size_bound: Option<NonZeroUsize>,
}

impl<T> FixedSizedInner<T> where T: Clone + Ord {
    fn new(capacity: Option<NonZeroUsize>) -> Self {
        Self {
            inner: Default::default(),
            size_bound: capacity,
        }
    }

    fn enforce_bound(&mut self) {
        if let Some(b) = self.size_bound {
            let bound = b.get();
            if bound < self.inner.len() {
                self.inner.slice(0..(self.inner.len() - bound));
            }
        }
    }

    pub fn partitioned(&self, at: &T) -> (im::Vector<T>, im::Vector<T>) {
        let out: im::Vector<T> = self.inner.clone();
        let loc = out.binary_search(at).coalesce();
        out.split_at(loc)
    }

    pub(crate) fn invariants_satisfied(&self) -> bool {
        self.size_bound.map(|b| b.get() >= self.inner.len())
            .unwrap_or(true) && self.unique_and_ordered()
    }

    fn unique_and_ordered(&self) -> bool {
        self.inner.iter().tuple_windows().all(|(a, b)| {
            a < b
        })
    }
}


pub struct OrdSet<T: Ord + Clone> {
    inner: RwLock<ArcSwap<FixedSizedInner<T>>>
}

impl<T: Ord + Clone> Clone for OrdSet<T> {
    fn clone(&self) -> Self {
        let tree = self.inner.read().load_full();
        Self::from_inner(tree)
    }
}

impl<T> fmt::Display for OrdSet<T> where T: Ord + Clone + fmt::Display {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let i = self.snapshot();
        f.debug_set()
            .entries(i.iter().map(ToString::to_string))
            .finish()
    }
}

impl<T: Ord + Clone> OrdSet<T> {
    fn from_inner(i: Arc<FixedSizedInner<T>>) -> Self {
        Self {
            inner: RwLock::new(ArcSwap::new(i))
        }
    }
}

impl<T: Ord + Clone> OrdSet<T> {
    pub fn new(bound: Option<NonZeroUsize>) -> Self {
        Self::from_inner(Arc::new(FixedSizedInner::new(bound)))
    }

    fn insert_inner(s: &mut im::Vector<T>, v: T) -> bool {
        match s.binary_search(&v) {
            Ok(idx) => {
                s.insert(idx, v);
                true
            }
            Err(idx) => {
                s.insert(idx, v);
                false
            }
        }
    }

    /// Inserts a new element, true if it wasn't already present.
    /// O(log n)
    pub fn insert(&self, v: T) -> bool {
        let ug = self.inner.read();
        let mut out = false;
        ug.rcu(|inner| {
            let mut inner = FixedSizedInner::clone(&inner);
            out = Self::insert_inner(&mut inner, v.clone());
            inner.enforce_bound();
            debug_assert!(inner.invariants_satisfied());
            Arc::new(inner)
        });
        out
    }

    /// Inserts many new elements, avoiding taking the lock each time. O(n log n)
    pub fn insert_all(&self, i: impl Iterator<Item=T>) -> usize {
        let wg = self.inner.write();
        let inner = wg.load_full();
        let mut inner = FixedSizedInner::clone(&inner);
        let out = i.map(|item| Self::insert_inner(&mut inner, item))
            .filter(|b| *b)
            .count();
        inner.enforce_bound();
        debug_assert!(inner.invariants_satisfied());
        wg.store(Arc::new(inner));
        out
    }

    pub fn remove(&self, v: &T) -> bool {
        let ug = self.inner.read();
        let mut out = false;

        ug.rcu(|inner| {
            let mut inner = FixedSizedInner::clone(inner);
            out = if let Ok(i) = inner.binary_search(v) {
                inner.remove(i);
                debug_assert!(inner.invariants_satisfied());
                true
            } else {
                false
            };
            Arc::new(inner)
        });

        out
    }

    pub fn remove_all_leq(&self, v: &T) -> im::Vector<T> {
        let g = self.inner.read();
        let mut left_half = im::Vector::new();
        g.rcu(|inner| {
            let (l, r) = inner.partitioned(v);
            left_half = l;
            Arc::new(FixedSizedInner {
                inner: r,
                size_bound: inner.size_bound
            })
        });
        left_half
    }

    pub fn remove_all_gt(&self, v: &T) -> im::Vector<T> {
        let g = self.inner.read();
        let mut right_half = im::Vector::new();
        g.rcu(|inner| {
            let (l, r) = inner.partitioned(v);
            right_half = r;
            Arc::new(FixedSizedInner {
                inner: l,
                size_bound: inner.size_bound
            })
        });
        right_half
    }

    pub fn remove_all<I, BT>(&self, i: I) -> usize where BT: Borrow<T>, I: Iterator<Item=BT> {
        let ug = self.inner.write();
        let inner = ug.load_full();
        let mut ninner = FixedSizedInner::clone(&inner);
        let o = i.map(|r| {
            let i = r.borrow();
            if let Ok(idx) = ninner.binary_search(i) {
                ninner.remove(idx);
                true
            } else {
                false
            }
        })
            .filter(|b| *b)
            .count();
        ug.store(Arc::new(ninner));
        o
    }

    pub fn partitioned(&self, at: &T) -> (im::Vector<T>, im::Vector<T>) {
        let inner = self.inner.read().load_full();
        inner.partitioned(at)
    }

    pub fn snapshot(&self) -> im::Vector<T> {
        let inner = self.inner.read().load_full();
        im::Vector::clone(&inner)
    }

    pub fn contains(&self, v: &T) -> bool {
        let rg = self.inner.read().load_full();
        rg.contains(v)
    }
}

impl<T> FromIterator<T> for OrdSet<T> where T: Ord + Clone {
    fn from_iter<I: IntoIterator<Item=T>>(iter: I) -> Self {
        let out = Self::from_inner(Arc::new(FixedSizedInner::new(None)));
        out.insert_all(iter.into_iter());
        out
    }
}