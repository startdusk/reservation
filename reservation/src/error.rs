use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReservationError {
    #[error("Database error")]
    DbError(#[from] sqlx::Error),

    #[error("Invalid start or end time for the reservation")]
    InvalidTime,

    #[error("Unknown error")]
    Unknown,
}
