mod config;
mod error;
mod pb;
mod types;
mod utils;

pub use config::*;
pub use utils::*;

pub use error::{Error, ReservationConflict, ReservationConflictInfo, ReservationWindow};
pub use pb::*;

pub trait Validator {
    fn validate(&self) -> Result<(), Error>;
}

/// database equivalent of the "reservation_status" enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "reservation_status", rename_all = "lowercase")]
pub enum RsvpStatus {
    Unknown,
    Confirmed,
    Pending,
    Blocked,
}

pub type ReservationId = i64;

impl Validator for ReservationId {
    fn validate(&self) -> Result<(), Error> {
        if *self <= 0 {
            return Err(Error::InvalidReservationId(*self));
        }

        Ok(())
    }
}

pub type UserId = String;

pub type ResourceId = String;
