mod error;
mod manager;

use async_trait::async_trait;
use sqlx::PgPool;

pub use error::ReservationError;

pub type ReservationId = String;

#[derive(Debug)]
pub struct ReservationManager {
    pool: PgPool,
}

#[async_trait]
pub trait Rsvp {
    /// make a reservation
    async fn reservation(
        &self,
        mut rsvp: abi::Reservation,
    ) -> Result<abi::Reservation, ReservationError>;
    /// change reservation status (if current status is pending, change it to confirmed)
    async fn change_status(&self, id: ReservationId) -> Result<abi::Reservation, ReservationError>;
    /// update note
    async fn update_note(
        &self,
        id: ReservationId,
        note: String,
    ) -> Result<abi::Reservation, ReservationError>;
    /// delete reservation
    async fn delete(&self, id: ReservationId) -> Result<(), ReservationError>;
    /// get reservation by id
    async fn get(&self, id: ReservationId) -> Result<abi::Reservation, ReservationError>;
    /// query reservation
    async fn query(
        &self,
        query: abi::ReservationQuery,
    ) -> Result<Vec<abi::Reservation>, ReservationError>;
}
