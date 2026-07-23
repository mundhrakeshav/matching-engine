mod order;
mod trade;

use std::fmt;
use std::ops::{AddAssign, SubAssign};

pub use order::{Order, OrderError, OrderKind, OrderSide, RestingOrder};
pub use trade::Trade;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct OrderId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct UserId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Sequence(pub u64);

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct Price(pub i64);

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    Default,
)]
#[serde(transparent)]
pub struct Quantity(pub u64);

impl fmt::Display for OrderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl OrderId {
    pub const fn into_inner(self) -> u64 {
        self.0
    }
}

impl UserId {
    pub const fn into_inner(self) -> u64 {
        self.0
    }
}

impl Sequence {
    pub const fn into_inner(self) -> u64 {
        self.0
    }

    pub const fn checked_add(self, rhs: u64) -> Option<Self> {
        match self.0.checked_add(rhs) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    pub const fn from(u: u64) -> Self {
        Self(u)
    }
}

impl Price {
    pub const fn into_inner(self) -> i64 {
        self.0
    }
}

impl Quantity {
    pub const fn into_inner(self) -> u64 {
        self.0
    }

    pub const fn from(u: u64) -> Self {
        Self(u)
    }

    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        match self.0.checked_sub(rhs.0) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

impl AddAssign for Quantity {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl SubAssign for Quantity {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl From<u64> for OrderId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<OrderId> for u64 {
    fn from(value: OrderId) -> Self {
        value.0
    }
}

impl From<u64> for UserId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<UserId> for u64 {
    fn from(value: UserId) -> Self {
        value.0
    }
}

impl From<u64> for Sequence {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<Sequence> for u64 {
    fn from(value: Sequence) -> Self {
        value.0
    }
}

impl From<i64> for Price {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl From<Price> for i64 {
    fn from(value: Price) -> Self {
        value.0
    }
}

impl From<u64> for Quantity {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<Quantity> for u64 {
    fn from(value: Quantity) -> Self {
        value.0
    }
}
