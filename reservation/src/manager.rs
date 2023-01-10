use abi::{Normalizer, ToSql, Validator};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::Row;
use sqlx::{postgres::types::PgRange, Either};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tracing::{info, warn};

use crate::{ReservationId, ReservationManager, Rsvp};

#[async_trait]
impl Rsvp for ReservationManager {
    /// make a reservation
    async fn reserve(&self, mut rsvp: abi::Reservation) -> Result<abi::Reservation, abi::Error> {
        rsvp.validate()?;

        let status = abi::ReservationStatus::from_i32(rsvp.status)
            .unwrap_or(abi::ReservationStatus::Pending);

        let timespan: PgRange<DateTime<Utc>> = rsvp.get_timespan();

        // generate a insert sql for the reservation
        // execute the sql
        let id: i64 = sqlx::query(
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

        rsvp.id = id;
        Ok(rsvp)
    }

    /// change reservation status (if current status is pending, change it to confirmed, otherwise do nothing)
    async fn change_status(&self, id: ReservationId) -> Result<abi::Reservation, abi::Error> {
        id.validate()?;
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
        id.validate()?;
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
    async fn delete(&self, id: ReservationId) -> Result<abi::Reservation, abi::Error> {
        id.validate()?;
        let rsvp: abi::Reservation = sqlx::query_as(
            r#"
                DELETE FROM rsvp.reservations WHERE id = $1
                RETURNING *
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(rsvp)
    }
    /// get reservation by id
    async fn get(&self, id: ReservationId) -> Result<abi::Reservation, abi::Error> {
        id.validate()?;
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
        query: abi::ReservationQuery,
    ) -> mpsc::Receiver<Result<abi::Reservation, abi::Error>> {
        let pool = self.pool.clone();
        let (tx, rx) = mpsc::channel(128);
        tokio::spawn(async move {
            let sql = query.to_sql();
            let mut rsvps = sqlx::query_as(&sql).fetch_many(&pool);
            while let Some(ret) = rsvps.next().await {
                match ret {
                    Ok(Either::Left(r)) => {
                        info!("Query result: {:?}", r);
                    }
                    Ok(Either::Right(r)) => {
                        if tx.send(Ok(r)).await.is_err() {
                            // rx is dropped, so client disconnected.
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("Query error: {:?}", e);
                        if tx.send(Err(e.into())).await.is_err() {
                            break;
                        }
                        break;
                    }
                }
            }
        });
        rx
    }

    /// filter reservations by user_id, resource_id, status, and order by id
    async fn filter(
        &self,
        mut filter: abi::ReservationFilter,
    ) -> Result<(abi::FilterPager, Vec<abi::Reservation>), abi::Error> {
        filter.normalize()?;
        let sql = filter.to_sql();
        let rsvps: Vec<abi::Reservation> = sqlx::query_as(&sql).fetch_all(&self.pool).await?;
        let mut data = rsvps.into_iter().collect();
        let pager = filter.get_pager(&mut data);
        Ok((pager, data.into_iter().collect()))
    }
}

#[cfg(test)]
mod tests {
    use abi::{
        Reservation, ReservationConflict, ReservationConflictInfo, ReservationFilterBuilder,
        ReservationQueryBuilder, ReservationStatus, ReservationWindow,
    };
    use docker_tester::TestPostgres;
    use prost_types::Timestamp;
    use sqlx::PgPool;

    use super::*;

    #[tokio::test]
    async fn reserve_should_work_for_valid_window() {
        let test_postgres = TestPostgres::new("../migrations").await.unwrap();
        let pool = test_postgres.get_pool().await;
        let (rsvp, _) = make_user_one_reservation(pool).await;
        assert!(rsvp.id != 0);
    }

    #[tokio::test]
    async fn reserve_conflict_reservation_should_reject() {
        let test_postgres = TestPostgres::new("../migrations").await.unwrap();
        let pool = test_postgres.get_pool().await;
        let manager = ReservationManager::new(pool);
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

    #[tokio::test]
    async fn reserve_change_status_should_work() {
        let test_postgres = TestPostgres::new("../migrations").await.unwrap();
        let pool = test_postgres.get_pool().await;
        let (rsvp, manager) = make_user_one_reservation(pool).await;
        assert!(rsvp.id != 0);

        let rsvp = manager.change_status(rsvp.id).await.unwrap();
        assert_eq!(rsvp.status, abi::ReservationStatus::Confirmed as i32);
    }

    #[tokio::test]
    async fn reserve_change_status_not_pending_should_do_nothing() {
        let test_postgres = TestPostgres::new("../migrations").await.unwrap();
        let pool = test_postgres.get_pool().await;
        let (rsvp, manager) = make_user_two_reservation(pool).await;
        assert!(rsvp.id != 0);

        let rsvp = manager.change_status(rsvp.id).await.unwrap();
        // change status again should do nothing
        let err = manager.change_status(rsvp.id).await.unwrap_err();
        assert_eq!(err, abi::Error::NotFound);
    }

    #[tokio::test]
    async fn update_note_should_work() {
        let test_postgres = TestPostgres::new("../migrations").await.unwrap();
        let pool = test_postgres.get_pool().await;
        let (rsvp, manager) = make_user_one_reservation(pool).await;
        let rsvp = manager
            .update_note(rsvp.id, "hello world".into())
            .await
            .unwrap();
        assert_eq!(rsvp.note, "hello world");
    }

    #[tokio::test]
    async fn get_reservation_should_work() {
        let test_postgres = TestPostgres::new("../migrations").await.unwrap();
        let pool = test_postgres.get_pool().await;
        let (new_rsvp, manager) = make_user_one_reservation(pool).await;
        let get_rsvp = manager.get(new_rsvp.id).await.unwrap();
        assert_eq!(new_rsvp, get_rsvp);
    }

    #[tokio::test]
    async fn delete_reservation_should_work() {
        let test_postgres = TestPostgres::new("../migrations").await.unwrap();
        let pool = test_postgres.get_pool().await;
        let (rsvp, manager) = make_user_one_reservation(pool).await;
        manager.delete(rsvp.id).await.unwrap();
        let err = manager.get(rsvp.id).await.unwrap_err();
        assert_eq!(err, abi::Error::NotFound);
    }

    #[tokio::test]
    async fn query_reservations_should_work() {
        let test_postgres = TestPostgres::new("../migrations").await.unwrap();
        let pool = test_postgres.get_pool().await;
        let (rsvp, manager) = make_user_one_reservation(pool).await;
        assert!(rsvp.id != 0);

        let query = ReservationQueryBuilder::default()
            .user_id("user_id_1")
            .resource_id("ocean-view-room-713")
            .start("2022-12-25T15:00:00-0700".parse::<Timestamp>().unwrap())
            .end("2022-12-28T12:00:00-0700".parse::<Timestamp>().unwrap())
            .status(ReservationStatus::Pending)
            .build()
            .unwrap();

        let mut rx = manager.query(query).await;
        assert_eq!(rx.recv().await.unwrap(), Ok(rsvp.clone()));
        assert_eq!(rx.recv().await, None);

        // if window is not in range, should return empty
        let query = ReservationQueryBuilder::default()
            .user_id("user_id_1")
            .resource_id("ocean-view-room-713")
            .start("2023-12-25T15:00:00-0700".parse::<Timestamp>().unwrap())
            .end("2023-12-28T12:00:00-0700".parse::<Timestamp>().unwrap())
            .status(ReservationStatus::Pending)
            .build()
            .unwrap();
        let mut rx = manager.query(query).await;
        assert_eq!(rx.recv().await, None);

        // if status is not in correct, should return empty
        let query = ReservationQueryBuilder::default()
            .user_id("user_id_1")
            .resource_id("ocean-view-room-713")
            .start("2022-12-25T15:00:00-0700".parse::<Timestamp>().unwrap())
            .end("2022-12-28T12:00:00-0700".parse::<Timestamp>().unwrap())
            .status(ReservationStatus::Confirmed)
            .build()
            .unwrap();
        let mut rx = manager.query(query.clone()).await;
        assert_eq!(rx.recv().await, None);

        // change state to confirmed, query should get result
        let rsvp = manager.change_status(rsvp.id).await.unwrap();
        let mut rx = manager.query(query).await;
        assert_eq!(rx.recv().await.unwrap(), Ok(rsvp));
        assert_eq!(rx.recv().await, None);
    }

    #[tokio::test]
    async fn filter_reservations_should_work() {
        let test_postgres = TestPostgres::new("../migrations").await.unwrap();
        let pool = test_postgres.get_pool().await;
        let (rsvp, manager) = make_user_one_reservation(pool).await;
        assert!(rsvp.id != 0);

        let filter = ReservationFilterBuilder::default()
            .user_id("user_id_1")
            .resource_id("ocean-view-room-713")
            .status(ReservationStatus::Pending)
            .build()
            .unwrap();

        let (pager, rsvps) = manager.filter(filter).await.unwrap();
        assert_eq!(pager.next, None);
        assert_eq!(pager.prev, None);
        assert_eq!(rsvps.len(), 1);
        assert_eq!(rsvps[0], rsvp);
    }

    //==========================================================================
    // private none test function
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
