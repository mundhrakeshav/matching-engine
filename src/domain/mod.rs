mod order;
mod trade;

pub use order::{Order, OrderError, OrderKind, OrderSide, RestingOrder};
pub use trade::Trade;

pub type OrderId = u64;
pub type UserId = u64;
pub type Sequence = u64;
pub type Price = i64;
pub type Quantity = u64;
