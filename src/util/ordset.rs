use std::fmt;
use std::fmt::Formatter;
use parking_lot::RwLock;
use parking_lot::lock_api::RwLockUpgradableReadGuard;
use crate::util::CoalesceResultExt;
use itertools::Itertools;
use std::borrow::Borrow;
use std::num::NonZeroUsize;
use std::iter::FromIterator;

#[derive(shrinkwraprs::Shrinkwrap, Clone)]
#[shrinkwrap(mutable)]
struct FixedSizedInner<T> {
    #[shrinkwrap(main_field)] inner: im::Vector<T>,
    size_bound: Option<NonZeroUsize>
}

impl<T> FixedSizedInner<T> where T: Clone + Ord {

    fn new(capacity: Option<NonZeroUsize>) -> Self {
        Self {
            inner: Default::default(),
            size_bound: capacity
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
    inner: RwLock<FixedSizedInner<T>>
}

impl<T: Ord + Clone> Clone for OrdSet<T> {
    fn clone(&self) -> Self {
        let tree = self.inner.read().clone();
        Self::from_inner(tree)
    }
}

impl<T> fmt::Display for OrdSet<T> where T: Ord + Clone + fmt::Display {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let i = self.inner.read();
        f.debug_set()
            .entries(i.iter().map(ToString::to_string))
            .finish()
    }
}

impl<T: Ord + Clone> OrdSet<T> {
    fn from_inner(i: FixedSizedInner<T>) -> Self {
        Self {
            inner: RwLock::new(i)
        }
    }
}

impl<T: Ord + Clone> OrdSet<T> {

    pub fn new(bound: Option<NonZeroUsize>) -> Self {
        Self::from_inner(FixedSizedInner::new(bound))
    }

    fn insert_inner(s: &mut im::Vector<T>, v: T) -> Option<T> {
        match s.binary_search(&v) {
            Ok(i) => {
                s.get_mut(i).map(|r| std::mem::replace(r, v))
            }
            Err(i) => {
                s.insert(i, v);
                None
            }
        }
    }

    /// Inserts a new element, returning the old element if it exists.
    /// O(log n)
    pub fn insert(&self, v: T) -> Option<T> {
        let ug = self.inner.upgradable_read();
        let mut inner = FixedSizedInner::clone(&ug);
        let out = Self::insert_inner(&mut inner, v);
        inner.enforce_bound();
        let mut wg = RwLockUpgradableReadGuard::upgrade(ug);
        debug_assert!(inner.invariants_satisfied());
        *wg = inner;
        out
    }

    /// Inserts many new elements, avoiding taking the lock each time. O(n log n)
    pub fn insert_all(&self, i: impl Iterator<Item=T>) -> Vec<T> {
        let mut wg = self.inner.write();
        let out = i.filter_map(|item| Self::insert_inner(&mut wg, item))
            .collect_vec();
        wg.enforce_bound();
        debug_assert!(wg.invariants_satisfied());
        out
    }

    pub fn remove(&self, v: &T) -> Option<T> {
        let ug = self.inner.upgradable_read();
        if let Ok(i) = ug.binary_search(v) {
            let mut wg = RwLockUpgradableReadGuard::upgrade(ug);
            let v = Some(wg.remove(i));
            debug_assert!(wg.invariants_satisfied());
            v
        } else {
            None
        }
    }

    pub fn remove_all_leq(&self, v: &T) -> im::Vector<T> {
        let ug = self.inner.upgradable_read();
        let mut c = im::Vector::clone(&ug);
        let split_idx = c.binary_search(v).coalesce();
        let n = c.split_off(split_idx);
        if !c.is_empty() {
            let mut wg = RwLockUpgradableReadGuard::upgrade(ug);
            *wg.as_mut() = n;
            debug_assert!(wg.invariants_satisfied());
        }
        c
    }

    pub fn remove_all_gt(&self, v: &T) -> im::Vector<T> {
        let ug = self.inner.upgradable_read();
        let mut c = im::Vector::clone(&ug);
        let split_idx = c.binary_search(v).coalesce();
        let n = c.split_off(split_idx);
        if !n.is_empty() {
            let mut wg = RwLockUpgradableReadGuard::upgrade(ug);
            *wg.as_mut() = c;
            debug_assert!(wg.invariants_satisfied());
        }
        n
    }

    pub fn remove_all<I, BT>(&self, i: I) -> usize where BT: Borrow<T>, I: Iterator<Item=BT> {
        let ug = self.inner.upgradable_read();
        let mut c = im::Vector::clone(&ug);
        let removed = i.filter_map(|item| {
            let r = item.borrow();
            if let Ok(i) = c.binary_search(r) {
                c.remove(i);
                Some(())
            } else {
                None
            }
        }).count();
        if removed > 0 {
            let mut wg = RwLockUpgradableReadGuard::upgrade(ug);
            *wg.as_mut() = c;
            debug_assert!(wg.invariants_satisfied());
        }
        removed
    }

    pub fn partitioned(&self, at: &T) -> (im::Vector<T>, im::Vector<T>) {
        let rg = self.inner.read();
        let out: im::Vector<T> = rg.as_ref().clone();
        std::mem::drop(rg);
        let loc = out.binary_search(at).coalesce();
        out.split_at(loc)
    }

    pub fn snapshot(&self) -> im::Vector<T> {
        self.inner.read().as_ref().clone()
    }

}

impl<T> FromIterator<T> for OrdSet<T> where T: Ord + Clone {
    fn from_iter<I: IntoIterator<Item=T>>(iter: I) -> Self {
        let out = Self::from_inner(FixedSizedInner::new(None));
        out.insert_all(iter.into_iter());
        out
    }
}