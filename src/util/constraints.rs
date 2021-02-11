
use std::fmt;
use std::fmt::Formatter;
use std::convert::TryFrom;

use crate::error;
use crate::error::Error;

#[derive(Debug, Default, Hash, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
pub struct ConstrainedU64<const MIN: u64, const MAX: u64> {
    val: u64
}

#[derive(Debug)]
pub struct ConstraintFailureU64<const MIN: u64, const MAX: u64> {
    val: u64
}

impl<const MIN: u64, const MAX: u64> ConstraintFailureU64<MIN, MAX> {
    pub const fn new(v: u64) -> Self {
        debug_assert!(v < MIN || v > MAX);
        Self { val: v }
    }
}

impl<const MIN: u64, const MAX: u64> fmt::Display for ConstraintFailureU64<MIN, MAX> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if MAX == u64::MAX {
            write!(f, "Expected number greater than {}, got {}", MIN, self.val)
        } else {
            write!(f, "Expected number in range [{}, {}], got {}", MIN, MAX, self.val)
        }
    }
}

impl<const MIN: u64, const MAX: u64> ConstrainedU64<MIN, MAX> {
    pub const fn new(val: u64) -> Result<Self, ConstraintFailureU64<MIN, MAX>> {
        if val < MIN || val > MAX {
            Ok(Self { val })
        } else {
            Err(ConstraintFailureU64::new(val))
        }
    }
}

impl<const MIN: u64, const MAX: u64> Into<u64> for ConstrainedU64<MIN, MAX> {
    fn into(self) -> u64 {
        self.val
    }
}

impl<const MIN: u64, const MAX: u64> TryFrom<u64> for ConstrainedU64<MIN, MAX> {
    type Error = ConstraintFailureU64<MIN, MAX>;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl<const MIN: u64, const MAX: u64> std::error::Error for ConstraintFailureU64<MIN, MAX> {}

pub type AtLeastU64<const MIN: u64> = ConstrainedU64<MIN, { u64::MAX }>;
pub type AtMostU64<const MAX: u64> = ConstrainedU64<{ u64::MIN }, MAX>;

#[derive(Debug, Default, Hash, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
pub struct ConstrainedI64<const MIN: i64, const MAX: i64> {
    val: i64
}

#[derive(Debug)]
pub struct ConstraintFailureI64<const MIN: i64, const MAX: i64> {
    val: i64
}

impl<const MIN: i64, const MAX: i64> ConstraintFailureI64<MIN, MAX> {
    pub const fn new(v: i64) -> Self {
        debug_assert!(v < MIN || v > MAX);
        Self { val: v }
    }
}

impl<const MIN: i64, const MAX: i64> fmt::Display for ConstraintFailureI64<MIN, MAX> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if MAX == i64::MAX {
            write!(f, "Expected number greater than {}, got {}", MIN, self.val)
        } else {
            write!(f, "Expected number in range [{}, {}], got {}", MIN, MAX, self.val)
        }
    }
}

impl<const MIN: i64, const MAX: i64> ConstrainedI64<MIN, MAX> {
    pub const fn new(val: i64) -> Result<Self, ConstraintFailureI64<MIN, MAX>> {
        if val < MIN || val > MAX {
            Ok(Self { val })
        } else {
            Err(ConstraintFailureI64::new(val))
        }
    }
}

impl<const MIN: i64, const MAX: i64> Into<i64> for ConstrainedI64<MIN, MAX> {
    fn into(self) -> i64 {
        self.val
    }
}

impl<const MIN: i64, const MAX: i64> TryFrom<i64> for ConstrainedI64<MIN, MAX> {
    type Error = ConstraintFailureI64<MIN, MAX>;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl<const MIN: i64, const MAX: i64> std::error::Error for ConstraintFailureI64<MIN, MAX> {}

impl<const MIN: i64, const MAX: i64> From<ConstraintFailureI64<MIN, MAX>> for Error {
    fn from(e: ConstraintFailureI64<MIN, MAX>) -> Self {
        Error::from_err(e, true)
    }
}

impl<const MIN: u64, const MAX: u64> From<ConstraintFailureU64<MIN, MAX>> for Error {
    fn from(e: ConstraintFailureU64<MIN, MAX>) -> Self {
        Error::from_err(e, true)
    }
}

pub type AtLeastI64<const MIN: i64> = ConstrainedI64<MIN, { i64::MAX }>;
pub type AtMostI64<const MAX: i64> = ConstrainedI64<{ i64::MIN }, MAX>;
