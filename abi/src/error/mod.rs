mod conflict;

use sqlx::postgres::PgDatabaseError;
use thiserror::Error;

pub use conflict::{ReservationConflict, ReservationConflictInfo, ReservationWindow};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Database error")]
    DbError(sqlx::Error),

    #[error("Failed to read configuration file")]
    ConfigReadError,

    #[error("Failed to parse configuration file")]
    ConfigParseError,

    #[error("Invalid start or end time for the reservation")]
    InvalidTime,

    #[error("Invalid user id: {0}")]
    InvalidUserId(String),

    #[error("Invalid reservation id: {0}")]
    InvalidReservationId(i64),

    #[error("No reservation found by the given condition")]
    NotFound,

    #[error("Conflict reservation")]
    ConflictReservation(ReservationConflictInfo),

    #[error("Invalid resource id: {0}")]
    InvalidResourceId(String),

    #[error("Unknown error")]
    Unknown,
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // TODO: this is not a good way to compare DB errors, but we don't do that in the code
            (Self::DbError(_), Self::DbError(_)) => true,
            (Self::InvalidUserId(v1), Self::InvalidUserId(v2)) => v1 == v2,
            (Self::InvalidReservationId(v1), Self::InvalidReservationId(v2)) => v1 == v2,
            (Self::ConflictReservation(v1), Self::ConflictReservation(v2)) => v1 == v2,
            (Self::InvalidResourceId(v1), Self::InvalidResourceId(v2)) => v1 == v2,
            (Self::NotFound, Self::NotFound) => true,
            (Self::Unknown, Self::Unknown) => true,
            (Self::InvalidTime, Self::InvalidTime) => true,
            _ => false,
        }
    }
}

impl From<sqlx::Error> for Error {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::Database(e) => {
                let err: &PgDatabaseError = e.downcast_ref();
                match (err.code(), err.schema(), err.table()) {
                    ("23P01", Some("rsvp"), Some("reservations")) => Error::ConflictReservation(
                        err.detail().unwrap().to_string().parse().unwrap(),
                    ),
                    _ => Error::DbError(sqlx::Error::Database(e)),
                }
            }
            sqlx::Error::RowNotFound => Error::NotFound,

            _ => Error::DbError(e),
        }
    }
}

impl From<Error> for tonic::Status {
    fn from(e: Error) -> Self {
        match e {
            Error::DbError(e) => tonic::Status::internal(format!("Database error: {}", e)),
            Error::ConfigReadError => {
                tonic::Status::internal("Failed to read configuration file".to_string())
            }
            Error::ConfigParseError => {
                tonic::Status::internal("Failed to parse configuration file".to_string())
            }
            Error::InvalidTime => {
                tonic::Status::invalid_argument("Invalid start or end time for the reservation")
            }
            Error::ConflictReservation(info) => {
                tonic::Status::failed_precondition(format!("Conflict reservation: {}", info))
            }
            Error::NotFound => tonic::Status::not_found("No reservation found by given condition"),
            Error::InvalidReservationId(id) => {
                tonic::Status::invalid_argument(format!("Invalid reservation id: {}", id))
            }
            Error::InvalidUserId(user_id) => {
                tonic::Status::invalid_argument(format!("Invalid user id: {}", user_id))
            }
            Error::InvalidResourceId(resource_id) => {
                tonic::Status::invalid_argument(format!("Invalid resource id: {}", resource_id))
            }
            Error::Unknown => tonic::Status::unknown("unknown error"),
        }
    }
}
