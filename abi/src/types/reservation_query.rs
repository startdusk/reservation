use chrono::{DateTime, Utc};
use sqlx::postgres::types::PgRange;

use crate::{convert_to_timestamp, Error, ReservationQuery, ReservationStatus, Validator};

use super::{get_timespan, validate_range};

impl ReservationQuery {
    pub fn new(
        uid: impl Into<String>,
        rid: impl Into<String>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        status: ReservationStatus,
        page: i32,
        page_size: i32,
        desc: bool,
    ) -> Self {
        Self {
            resource_id: rid.into(),
            user_id: uid.into(),
            status: status as i32,
            start: Some(convert_to_timestamp(start)),
            end: Some(convert_to_timestamp(end)),
            page,
            page_size,
            desc,
        }
    }

    pub fn get_timespan(&self) -> PgRange<DateTime<Utc>> {
        get_timespan(self.start.as_ref(), self.end.as_ref())
    }
}

impl Validator for ReservationQuery {
    fn validate(&self) -> Result<(), Error> {
        validate_range(self.start.as_ref(), self.end.as_ref())?;
        Ok(())
    }
}
