use chrono::{DateTime, Utc};
use sqlx::postgres::types::PgRange;

use crate::{Error, Normalizer, ReservationQuery, ReservationQueryBuilder, Validator};

use super::{get_timespan, validate_range};

impl ReservationQueryBuilder {
    pub fn build(&self) -> Result<ReservationQuery, Error> {
        let mut query = self
            .private_build()
            .expect("failed to build ReservationFilter");
        query.normalize()?;
        Ok(query)
    }
}

impl ReservationQuery {
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

impl Normalizer for ReservationQuery {
    fn do_normalize(&mut self) {
        // do noting
    }
}
