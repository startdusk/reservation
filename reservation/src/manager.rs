use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::Row;
use sqlx::{postgres::types::PgRange, types::Uuid};

use crate::{ReservationId, ReservationManager, Rsvp};

#[async_trait]
impl Rsvp for ReservationManager {
    /// make a reservation
    async fn reserve(&self, mut rsvp: abi::Reservation) -> Result<abi::Reservation, abi::Error> {
        rsvp.validate()?;

        let status = abi::ReservationStatus::from_i32(rsvp.status)
            .unwrap_or(abi::ReservationStatus::Pending);

        let timespan: PgRange<DateTime<Utc>> = rsvp.get_timespan().into();

        // generate a insert sql for the reservation
        // execute the sql
        let id: Uuid = sqlx::query(
            r#"
                INSERT INTO rsvp.reservations (user_id, resource_id, timespan, note, status)
                VALUES ($1, $2, $3, $4, $5::rsvp.reservation_status) RETURNING id
            "#,
        )
        .bind(rsvp.user_id.clone())
        .bind(rsvp.resource_id.clone())
        .bind(timespan)
        .bind(rsvp.note.clone())
        .bind(status.to_string())
        .fetch_one(&self.pool)
        .await?
        .get(0);

        rsvp.id = id.to_string();
        Ok(rsvp)
    }

    /// change reservation status (if current status is pending, change it to confirmed, otherwise do nothing)
    async fn change_status(&self, id: ReservationId) -> Result<abi::Reservation, abi::Error> {
        let id = Uuid::parse_str(&id).map_err(|_| abi::Error::InvalidReservationId(id.clone()))?;
        let rsvp: abi::Reservation = sqlx::query_as(
            r#"
                UPDATE rsvp.reservations SET status = 'confirmed' WHERE id = $1 AND status = 'pending'
                RETURNING *
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool).await?;

        Ok(rsvp)
    }

    /// update note
    async fn update_note(
        &self,
        id: ReservationId,
        note: String,
    ) -> Result<abi::Reservation, abi::Error> {
        let id = Uuid::parse_str(&id).map_err(|_| abi::Error::InvalidReservationId(id.clone()))?;
        let rsvp: abi::Reservation = sqlx::query_as(
            r#"
                UPDATE rsvp.reservations SET note = $1 WHERE id = $2
                RETURNING *
            "#,
        )
        .bind(note)
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(rsvp)
    }

    /// delete reservation
    async fn delete(&self, id: ReservationId) -> Result<(), abi::Error> {
        let id = Uuid::parse_str(&id).map_err(|_| abi::Error::InvalidReservationId(id.clone()))?;
        sqlx::query(
            r#"
                DELETE FROM rsvp.reservations WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
    /// get reservation by id
    async fn get(&self, id: ReservationId) -> Result<abi::Reservation, abi::Error> {
        let id = Uuid::parse_str(&id).map_err(|_| abi::Error::InvalidReservationId(id.clone()))?;
        let rsvp: abi::Reservation = sqlx::query_as(
            r#"
                SELECT * FROM rsvp.reservations WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(rsvp)
    }
    /// query reservation
    async fn query(
        &self,
        _query: abi::ReservationQuery,
    ) -> Result<Vec<abi::Reservation>, abi::Error> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use abi::{Reservation, ReservationConflict, ReservationConflictInfo, ReservationWindow};
    use sqlx::PgPool;

    use super::*;

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn reserve_should_work_for_valid_window() {
        let manager = ReservationManager::new(migrated_pool.clone());
        let rsvp = abi::Reservation::new_pending(
            "user_id",
            "ocean-view-room-713",
            "2022-12-25T15:00:00-0700".parse().unwrap(),
            "2022-12-28T12:00:00-0700".parse().unwrap(),
            "I'll arrive at 3pm. Please help to upgrade to execuitive room if possible.",
        );
        let rsvp = manager.reserve(rsvp).await.unwrap();
        assert!(rsvp.id != "");
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn reserve_conflict_reservation_should_reject() {
        let manager = ReservationManager::new(migrated_pool.clone());
        let rsvp1 = abi::Reservation::new_pending(
            "user_id_1",
            "ocean-view-room-713",
            "2022-12-25T15:00:00-0700".parse().unwrap(),
            "2022-12-28T12:00:00-0700".parse().unwrap(),
            "hello I'm user 1.",
        );
        let rsvp2 = abi::Reservation::new_pending(
            "user_id_2",
            "ocean-view-room-713",
            "2022-12-26T15:00:00-0700".parse().unwrap(),
            "2022-12-30T12:00:00-0700".parse().unwrap(),
            "hello I'm user 2.",
        );
        let _rsvp1 = manager.reserve(rsvp1).await.unwrap();
        let err = manager.reserve(rsvp2).await.unwrap_err();

        let info = ReservationConflictInfo::Parsed(ReservationConflict {
            new: ReservationWindow {
                rid: "ocean-view-room-713".to_string(),
                start: "2022-12-26T15:00:00-0700".parse().unwrap(),
                end: "2022-12-30T12:00:00-0700".parse().unwrap(),
            },
            old: ReservationWindow {
                rid: "ocean-view-room-713".to_string(),
                start: "2022-12-25T15:00:00-0700".parse().unwrap(),
                end: "2022-12-28T12:00:00-0700".parse().unwrap(),
            },
        });

        assert_eq!(err, abi::Error::ConflictReservation(info));
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn reserve_change_status_should_work() {
        let manager = ReservationManager::new(migrated_pool.clone());
        let rsvp = abi::Reservation::new_pending(
            "aliceid",
            "ixia-test-1",
            "2022-01-25T15:00:00-0700".parse().unwrap(),
            "2022-02-28T12:00:00-0700".parse().unwrap(),
            "I need to book this for xyz project for a month.",
        );
        let rsvp = manager.reserve(rsvp).await.unwrap();
        assert!(rsvp.id != "");

        let rsvp = manager.change_status(rsvp.id).await.unwrap();
        assert_eq!(rsvp.status, abi::ReservationStatus::Confirmed as i32);
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn reserve_change_status_not_pending_should_do_nothing() {
        let manager = ReservationManager::new(migrated_pool.clone());
        let rsvp = abi::Reservation::new_pending(
            "aliceid",
            "ixia-test-1",
            "2022-01-25T15:00:00-0700".parse().unwrap(),
            "2022-02-28T12:00:00-0700".parse().unwrap(),
            "I need to book this for xyz project for a month.",
        );
        let rsvp = manager.reserve(rsvp).await.unwrap();
        assert!(rsvp.id != "");

        let rsvp = manager.change_status(rsvp.id).await.unwrap();
        // change status again should do nothing
        let err = manager.change_status(rsvp.id).await.unwrap_err();
        assert_eq!(err, abi::Error::RowNotFound);
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn update_note_should_work() {
        let (rsvp, manager) = make_user_one_reservation(migrated_pool.clone()).await;
        let rsvp = manager
            .update_note(rsvp.id, "hello world".into())
            .await
            .unwrap();
        assert_eq!(rsvp.note, "hello world");
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn get_reservation_should_work() {
        let (new_rsvp, manager) = make_user_one_reservation(migrated_pool.clone()).await;
        let get_rsvp = manager.get(new_rsvp.id.clone()).await.unwrap();
        assert_eq!(new_rsvp, get_rsvp);
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn delete_reservation_should_work() {
        let (rsvp, manager) = make_user_one_reservation(migrated_pool.clone()).await;
        manager.delete(rsvp.id.clone()).await.unwrap();
        let err = manager.get(rsvp.id.clone()).await.unwrap_err();
        assert_eq!(err, abi::Error::RowNotFound);
    }

    async fn make_user_one_reservation(pool: PgPool) -> (Reservation, ReservationManager) {
        make_reservation(
            pool,
            "user_id_1",
            "ocean-view-room-713",
            "2022-12-25T15:00:00-0700",
            "2022-12-28T12:00:00-0700",
            "hello I'm user 1.",
        )
        .await
    }

    async fn make_user_two_reservation(pool: PgPool) -> (Reservation, ReservationManager) {
        make_reservation(
            pool,
            "user_id_2",
            "ocean-view-room-713",
            "2022-12-26T15:00:00-0700",
            "2022-12-30T12:00:00-0700",
            "hello I'm user 2.",
        )
        .await
    }

    async fn make_reservation(
        pool: PgPool,
        uid: &str,
        rid: &str,
        start: &str,
        end: &str,
        note: &str,
    ) -> (Reservation, ReservationManager) {
        let manager = ReservationManager::new(pool.clone());
        let rsvp = abi::Reservation::new_pending(
            uid,
            rid,
            start.parse().unwrap(),
            end.parse().unwrap(),
            note,
        );
        (manager.reserve(rsvp).await.unwrap(), manager)
    }
}
