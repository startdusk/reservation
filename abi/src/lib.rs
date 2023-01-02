mod config;
mod error;
mod pager;
mod pb;
mod types;
mod utils;

pub use config::*;
pub use utils::*;

pub use error::{Error, ReservationConflict, ReservationConflictInfo, ReservationWindow};
pub use pb::*;

/// validate the data structure, raise error if invalid
pub trait Validator {
    fn validate(&self) -> Result<(), Error>;
}

/// validate and normalize the data structure
pub trait Normalizer: Validator {
    /// caller should call normalize to make sure the data structure is ready to use
    fn normalize(&mut self) -> Result<(), Error> {
        self.validate()?;
        self.do_normalize();
        Ok(())
    }

    /// use shall implement do_normalize() to normalize the data structure
    fn do_normalize(&mut self);
}

pub trait ToSql {
    fn to_sql(&self) -> Result<String, Error>;
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
