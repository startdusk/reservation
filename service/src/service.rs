use abi::{
    reservation_service_server::ReservationService, CancelRequest, CancelResponse, Config,
    ConfirmRequest, ConfirmResponse, FilterRequest, FilterResponse, GetRequest, GetResponse,
    ListenRequest, QueryRequest, ReserveRequest, ReserveResponse, UpdateRequest, UpdateResponse,
};
use reservation::{ReservationManager, Rsvp};
use tonic::{Request, Response, Status};

use crate::{ReservationStream, RsvpService};

impl RsvpService {
    pub async fn from_config(config: &Config) -> Result<Self, anyhow::Error> {
        Ok(Self {
            manager: ReservationManager::from_config(&config.db).await?,
        })
    }
}

#[tonic::async_trait]
impl ReservationService for RsvpService {
    /// make a reservation
    async fn reserve(
        &self,
        request: Request<ReserveRequest>,
    ) -> Result<Response<ReserveResponse>, Status> {
        let request = request.into_inner();
        if request.reservation.is_none() {
            return Err(Status::invalid_argument("missing reservation"));
        }
        let reservation = self.manager.reserve(request.reservation.unwrap()).await?;
        Ok(Response::new(ReserveResponse {
            reservation: Some(reservation),
        }))
    }
    /// confirm a pending reservation, if reservation is not pending, do nothing
    async fn confirm(
        &self,
        _request: Request<ConfirmRequest>,
    ) -> Result<Response<ConfirmResponse>, Status> {
        todo!()
    }
    /// update the reservation note
    async fn update(
        &self,
        _request: Request<UpdateRequest>,
    ) -> Result<Response<UpdateResponse>, Status> {
        todo!()
    }
    /// cancel a reservation
    async fn cancel(
        &self,
        _request: Request<CancelRequest>,
    ) -> Result<Response<CancelResponse>, Status> {
        todo!()
    }
    /// get a reservation by id
    async fn get(&self, _request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        todo!()
    }
    ///Server streaming response type for the query method.
    type queryStream = ReservationStream;

    /// query reservations by resource id, user id, status, start time, end time
    async fn query(
        &self,
        _request: Request<QueryRequest>,
    ) -> Result<Response<Self::queryStream>, Status> {
        todo!()
    }
    /// filter reservations, order by reservatioin id
    async fn filter(
        &self,
        _request: Request<FilterRequest>,
    ) -> Result<Response<FilterResponse>, Status> {
        todo!()
    }
    ///Server streaming response type for the listen method.
    type listenStream = ReservationStream;

    /// another system could monitor newly added/confirmed/cancelled reservations
    async fn listen(
        &self,
        _request: Request<ListenRequest>,
    ) -> Result<Response<Self::listenStream>, Status> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use abi::{DbConfig, Reservation};
    use docker_tester::TestPostgres;

    #[tokio::test]
    async fn rpc_reserve_should_work() {
        let test_app = TestPostgres::new("../migrations").await.unwrap();
        let config = Config {
            db: DbConfig {
                host: test_app.host.clone(),
                port: test_app.port.clone(),
                user: test_app.user.clone(),
                password: test_app.password.clone(),
                dbname: test_app.dbname.clone(),
                max_connections: 5,
            },
            ..Default::default()
        };
        let service = RsvpService::from_config(&config).await.unwrap();
        let reservation = Reservation::new_pending(
            "ben",
            "ixia-3228",
            "2022-12-26T15:00:00-0700".parse().unwrap(),
            "2022-12-30T12:00:00-0700".parse().unwrap(),
            "test device reservation",
        );
        let request = tonic::Request::new(ReserveRequest {
            reservation: Some(reservation.clone()),
        });
        let response = service.reserve(request).await.unwrap();
        let reservation_resp = response.into_inner().reservation;
        assert!(reservation_resp.is_some());
        let insert_reservation = reservation_resp.unwrap();
        assert!(insert_reservation.id > 0);
        assert_eq!(insert_reservation.user_id, reservation.user_id);
        assert_eq!(insert_reservation.status, reservation.status);
        assert_eq!(insert_reservation.resource_id, reservation.resource_id);
        assert_eq!(insert_reservation.start, reservation.start);
        assert_eq!(insert_reservation.end, reservation.end);
        assert_eq!(insert_reservation.note, reservation.note);
    }
}
