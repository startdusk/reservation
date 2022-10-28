use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::postgres::types::PgRange;
use sqlx::Row;

use crate::{ReservationError, ReservationId, ReservationManager, Rsvp};

#[async_trait]
impl Rsvp for ReservationManager {
    /// make a reservation
    async fn reservation(
        &self,
        mut rsvp: abi::Reservation,
    ) -> Result<abi::Reservation, ReservationError> {
        if rsvp.start.is_none() || rsvp.end.is_none() {
            return Err(ReservationError::InvalidTime);
        }

        let start = abi::convert_to_utc_time(rsvp.start.as_ref().unwrap().clone());
        let end = abi::convert_to_utc_time(rsvp.end.as_ref().unwrap().clone());
        let timespan: PgRange<DateTime<Utc>> = (start..end).into();

        // generate a insert sql for the reservation
        // execute the sql
        let id = sqlx::query(
            r#"
                INSERT INTO reservation (user_id, resource_id, timespan, note, status)
                VALUES ($1, $2, $3, $4, $5) RETURNING id
            "#,
        )
        .bind(rsvp.user_id.clone())
        .bind(rsvp.resource_id.clone())
        .bind(timespan)
        .bind(rsvp.note.clone())
        .bind(rsvp.status)
        .fetch_one(&self.pool)
        .await?
        .get(0);

        rsvp.id = id;
        Ok(rsvp)
    }
    /// change reservation status (if current status is pending, change it to confirmed)
    async fn change_status(
        &self,
        _id: ReservationId,
    ) -> Result<abi::Reservation, ReservationError> {
        todo!()
    }
    /// update note
    async fn update_note(
        &self,
        _id: ReservationId,
        _note: String,
    ) -> Result<abi::Reservation, ReservationError> {
        todo!()
    }
    /// delete reservation
    async fn delete(&self, _id: ReservationId) -> Result<(), ReservationError> {
        todo!()
    }
    /// get reservation by id
    async fn get(&self, _id: ReservationId) -> Result<abi::Reservation, ReservationError> {
        todo!()
    }
    /// query reservation
    async fn query(
        &self,
        _query: abi::ReservationQuery,
    ) -> Result<Vec<abi::Reservation>, ReservationError> {
        todo!()
    }
}
