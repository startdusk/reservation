use sqlx::postgres::PgDatabaseError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Database error")]
    DbError(sqlx::Error),

    #[error("Invalid start or end time for the reservation")]
    InvalidTime,

    #[error("Invalid user id: {0}")]
    InvalidUserId(String),

    #[error("{0}")]
    ConflictReservation(String),

    #[error("Invalid resource id: {0}")]
    InvalidResourceId(String),

    #[error("Unknown error")]
    Unknown,
}

impl From<sqlx::Error> for Error {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::Database(e) => {
                let err: &PgDatabaseError = e.downcast_ref();
                match (err.code(), err.schema(), err.table()) {
                    ("23P01", Some("rsvp"), Some("reservations")) => {
                        Error::ConflictReservation(err.detail().unwrap().to_string())
                    }
                    _ => Error::DbError(sqlx::Error::Database(e)),
                }
            }
            _ => Error::DbError(e),
        }
    }
}

// TODO: write a parser
// "Key (resource_id, timespan)=(ocean-view-room-713, [\"2022-12-26 22:00:00+00\",\"2022-12-30 19:00:00+00\")) conflicts with existing key (resource_id, timespan)=(ocean-view-room-713, [\"2022-12-25 22:00:00+00\",\"2022-12-28 19:00:00+00\"))."
// pub struct ReservationConflictInfo {
//     a: ReservationWindow,
//     b: ReservationWindow,
// }

// pub struct ReservationWindow {
//     rid: String,
//     start: DateTime<Utc>,
//     end: DateTime<Utc>,
// }
