use std::ops::Bound;

use chrono::{DateTime, Utc};
use prost_types::Timestamp;
use sqlx::postgres::types::PgRange;

use crate::{convert_to_utc_time, Error};

mod reservation;
mod reservation_query;
mod reservation_status;

pub fn validate_range(start: Option<&Timestamp>, end: Option<&Timestamp>) -> Result<(), Error> {
    if start.is_none() || end.is_none() {
        return Err(Error::InvalidTime);
    }

    let start = start.as_ref().unwrap();
    let end = end.as_ref().unwrap();

    if start.seconds >= end.seconds {
        return Err(Error::InvalidTime);
    }

    Ok(())
}

pub fn get_timespan(start: Option<&Timestamp>, end: Option<&Timestamp>) -> PgRange<DateTime<Utc>> {
    let start = convert_to_utc_time(start.unwrap().clone());
    let end = convert_to_utc_time(end.unwrap().clone());
    PgRange {
        start: Bound::Included(start),
        end: Bound::Excluded(end),
    }
}
