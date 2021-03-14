//! Contains constrained integer types, mostly useful as hard limits in module options.

#![allow(clippy::from_over_into)]

use std::convert::TryFrom;
use std::fmt;
use std::fmt::Formatter;
use std::str::FromStr;

use crate::error::Error;

/// A constrained unsigned 64-bit integer. It is guaranteed to be no more than `MAX` and no less than
/// `MIN`.
#[derive(Debug, Default, Hash, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
pub struct ConstrainedU64<const MIN: u64, const MAX: u64> {
    /// The contained value.
    val: u64
}

/// Represents a constraint failure, usually generated during parsing.
#[derive(Debug)]
pub struct ConstraintFailureU64<const MIN: u64, const MAX: u64> {
    /// The value which violated the type constraint.
    val: u64
}

impl<const MIN: u64, const MAX: u64> ConstraintFailureU64<MIN, MAX> {
    /// Creates a new error type. Contains a debug assertion that the value *actually* violates the type constraint.
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
    /// Creates a new constrained integer, returning an error if the value falls outside the constraint.
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

/// An alias for constrained unsigned integers where the value can be no less than `MIN`.
pub type AtLeastU64<const MIN: u64> = ConstrainedU64<MIN, { u64::MAX }>;
/// An alias for constrained unsigned integers where the value can be no more than `MAX`.
pub type AtMostU64<const MAX: u64> = ConstrainedU64<{ u64::MIN }, MAX>;

/// Similar to [`ConstrainedU64`], but for signed integers.
#[derive(Debug, Default, Hash, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
pub struct ConstrainedI64<const MIN: i64, const MAX: i64> {
    /// The contained value.
    val: i64
}

/// See [`ConstraintFailureU64`]. This variation is for signed integers.
#[derive(Debug)]
pub struct ConstraintFailureI64<const MIN: i64, const MAX: i64> {
    /// The value which violated the constraint.
    val: i64
}

impl<const MIN: i64, const MAX: i64> ConstraintFailureI64<MIN, MAX> {
    /// Creates a new constraint failure.
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
    /// Creates a new constrained 64-bit signed integer, which is no more than `MAX` and no less
    /// than `MIN`. Returns an error if the value violates the constraints.
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

impl<const MIN: u64, const MAX: u64> FromStr for ConstrainedU64<MIN, MAX> {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u64>()
            .map_err(|e| Self::Err::from_err(e, true))
            .and_then(|u| Self::try_from(u).map_err(Error::from))
    }
}

/// Same as [`AtLeastU64`], but for signed values.
pub type AtLeastI64<const MIN: i64> = ConstrainedI64<MIN, { i64::MAX }>;
/// Same as [`AtMostU64`], but for signed values.
pub type AtMostI64<const MAX: i64> = ConstrainedI64<{ i64::MIN }, MAX>;
