mod error;
mod pb;
mod types;
mod utils;

pub use utils::*;

pub use error::{Error, ReservationConflict, ReservationConflictInfo, ReservationWindow};
pub use pb::*;

/// database equivalent of the "reservation_status" enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "reservation_status", rename_all = "lowercase")]
pub enum RsvpStatus {
    Unknown,
    Confirmed,
    Pending,
    Blocked,
}
