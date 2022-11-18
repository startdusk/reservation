use abi::{FilterPager, Validator};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::postgres::types::PgRange;
use sqlx::Row;

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
    async fn delete(&self, id: ReservationId) -> Result<(), abi::Error> {
        id.validate()?;
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
    ) -> Result<Vec<abi::Reservation>, abi::Error> {
        let user_id = str_to_option(&query.user_id);
        let resource_id = str_to_option(&query.resource_id);
        let range: PgRange<DateTime<Utc>> = query.get_timespan();
        let status = abi::ReservationStatus::from_i32(query.status)
            .unwrap_or(abi::ReservationStatus::Pending);
        let rsvps = sqlx::query_as(
            "SELECT * FROM rsvp.query($1, $2, $3, $4::rsvp.reservation_status, $5, $6, $7)",
        )
        .bind(user_id)
        .bind(resource_id)
        .bind(range)
        .bind(status.to_string())
        .bind(query.page)
        .bind(query.desc)
        .bind(query.page_size)
        .fetch_all(&self.pool)
        .await?;

        Ok(rsvps)
    }

    /// filter reservations by user_id, resource_id, status, and order by id
    async fn filter(
        &self,
        filter: abi::ReservationFilter,
    ) -> Result<(abi::FilterPager, Vec<abi::Reservation>), abi::Error> {
        let user_id = str_to_option(&filter.user_id);
        let resource_id = str_to_option(&filter.resource_id);
        let status = abi::ReservationStatus::from_i32(filter.status)
            .unwrap_or(abi::ReservationStatus::Pending);
        let page_size = if filter.page_size < 10 || filter.page_size > 100 {
            10
        } else {
            filter.page_size
        };
        let rsvps: Vec<abi::Reservation> = sqlx::query_as(
            "SELECT * FROM rsvp.filter($1, $2, $3::rsvp.reservation_status, $4, $5, $6)",
        )
        .bind(user_id)
        .bind(resource_id)
        .bind(status.to_string())
        .bind(filter.cursor)
        .bind(filter.desc)
        .bind(page_size)
        .fetch_all(&self.pool)
        .await?;

        // if the first id is current cursor, then we have prev, we start from 1
        // if (len - start) > page_size, then we have next, we end at len - 1
        let has_prev = !rsvps.is_empty() && rsvps[0].id == filter.cursor;
        // let start = if has_prev { 1 } else { 0 }; ==> usize::from(has_prev)
        let start = usize::from(has_prev);
        let has_next = (rsvps.len() - start) as i32 > page_size;
        let end = if has_next {
            rsvps.len() - 1
        } else {
            rsvps.len()
        };
        let prev = if has_prev { rsvps[start - 1].id } else { -1 };
        let next = if has_next { rsvps[end - 1].id } else { -1 };

        // TODO: optimize this clone
        let result = rsvps[start..end].to_vec();

        let pager = FilterPager {
            prev,
            next,
            // TODO: how to get total efficiently?
            total: 0,
        };
        Ok((pager, result))
    }
}

fn str_to_option(s: &str) -> Option<&str> {
    if s.is_empty() {
        return None;
    }

    Some(s)
}

#[cfg(test)]
mod tests {
    use abi::{
        Reservation, ReservationConflict, ReservationConflictInfo, ReservationFilterBuilder,
        ReservationQueryBuilder, ReservationStatus, ReservationWindow,
    };
    use prost_types::Timestamp;
    use sqlx::PgPool;

    use super::*;

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn reserve_should_work_for_valid_window() {
        let (rsvp, _) = make_user_one_reservation(migrated_pool.clone()).await;
        assert!(rsvp.id != 0);
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
        let (rsvp, manager) = make_user_one_reservation(migrated_pool.clone()).await;
        assert!(rsvp.id != 0);

        let rsvp = manager.change_status(rsvp.id).await.unwrap();
        assert_eq!(rsvp.status, abi::ReservationStatus::Confirmed as i32);
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn reserve_change_status_not_pending_should_do_nothing() {
        let (rsvp, manager) = make_user_two_reservation(migrated_pool.clone()).await;
        assert!(rsvp.id != 0);

        let rsvp = manager.change_status(rsvp.id).await.unwrap();
        // change status again should do nothing
        let err = manager.change_status(rsvp.id).await.unwrap_err();
        assert_eq!(err, abi::Error::NotFound);
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
        manager.delete(rsvp.id).await.unwrap();
        let err = manager.get(rsvp.id).await.unwrap_err();
        assert_eq!(err, abi::Error::NotFound);
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn query_reservations_should_work() {
        let (rsvp, manager) = make_user_one_reservation(migrated_pool.clone()).await;
        assert!(rsvp.id != 0);

        let query = ReservationQueryBuilder::default()
            .user_id("user_id_1")
            .resource_id("ocean-view-room-713")
            .start("2022-12-25T15:00:00-0700".parse::<Timestamp>().unwrap())
            .end("2022-12-28T12:00:00-0700".parse::<Timestamp>().unwrap())
            .status(ReservationStatus::Pending)
            .build()
            .unwrap();

        let rsvps = manager.query(query).await.unwrap();
        assert_eq!(rsvps.len(), 1);
        assert_eq!(rsvps[0], rsvp);

        // if window is not in range, should return empty
        let query = ReservationQueryBuilder::default()
            .user_id("user_id_1")
            .resource_id("ocean-view-room-713")
            .start("2023-12-25T15:00:00-0700".parse::<Timestamp>().unwrap())
            .end("2023-12-28T12:00:00-0700".parse::<Timestamp>().unwrap())
            .status(ReservationStatus::Pending)
            .build()
            .unwrap();
        let rsvps = manager.query(query).await.unwrap();
        assert_eq!(rsvps.len(), 0);

        // if status is not in correct, should return empty
        let query = ReservationQueryBuilder::default()
            .user_id("user_id_1")
            .resource_id("ocean-view-room-713")
            .start("2022-12-25T15:00:00-0700".parse::<Timestamp>().unwrap())
            .end("2022-12-28T12:00:00-0700".parse::<Timestamp>().unwrap())
            .status(ReservationStatus::Confirmed)
            .build()
            .unwrap();
        let rsvps = manager.query(query.clone()).await.unwrap();
        assert_eq!(rsvps.len(), 0);

        // change state to confirmed, query should get result
        let rsvp = manager.change_status(rsvp.id).await.unwrap();
        let rsvps = manager.query(query).await.unwrap();
        assert_eq!(rsvps.len(), 1);
        assert_eq!(rsvps[0], rsvp);
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn filter_reservations_should_work() {
        let (rsvp, manager) = make_user_one_reservation(migrated_pool.clone()).await;
        assert!(rsvp.id != 0);

        let filter = ReservationFilterBuilder::default()
            .user_id("user_id_1")
            .resource_id("ocean-view-room-713")
            .status(ReservationStatus::Pending)
            .build()
            .unwrap();

        let (pager, rsvps) = manager.filter(filter).await.unwrap();
        assert_eq!(pager.next, -1);
        assert_eq!(pager.prev, -1);
        assert_eq!(rsvps.len(), 1);
        assert_eq!(rsvps[0], rsvp);
    }

    //===================================================================================================================
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
