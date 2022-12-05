use std::task::Poll;

use abi::{
    reservation_service_server::ReservationService, CancelRequest, CancelResponse, Config,
    ConfirmRequest, ConfirmResponse, FilterRequest, FilterResponse, GetRequest, GetResponse,
    ListenRequest, QueryRequest, ReserveRequest, ReserveResponse, UpdateRequest, UpdateResponse,
};
use futures::Stream;
use reservation::{ReservationManager, Rsvp};
use tokio::sync::mpsc;
use tonic::{Request, Response, Status};

use crate::{ReservationStream, RsvpService, TonicReceiverStream};

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
        request: Request<ConfirmRequest>,
    ) -> Result<Response<ConfirmResponse>, Status> {
        let request = request.into_inner();
        let reservation = self.manager.change_status(request.id).await?;
        Ok(Response::new(ConfirmResponse {
            reservation: Some(reservation),
        }))
    }
    /// update the reservation note
    async fn update(
        &self,
        request: Request<UpdateRequest>,
    ) -> Result<Response<UpdateResponse>, Status> {
        let request = request.into_inner();
        let reservation = self.manager.update_note(request.id, request.note).await?;
        Ok(Response::new(UpdateResponse {
            reservation: Some(reservation),
        }))
    }
    /// cancel a reservation
    async fn cancel(
        &self,
        request: Request<CancelRequest>,
    ) -> Result<Response<CancelResponse>, Status> {
        let request = request.into_inner();
        let reservation = self.manager.delete(request.id).await?;
        Ok(Response::new(CancelResponse {
            reservation: Some(reservation),
        }))
    }
    /// get a reservation by id
    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let request = request.into_inner();
        let reservation = self.manager.get(request.id).await?;
        Ok(Response::new(GetResponse {
            reservation: Some(reservation),
        }))
    }
    ///Server streaming response type for the query method.
    type queryStream = ReservationStream;

    /// query reservations by resource id, user id, status, start time, end time
    async fn query(
        &self,
        request: Request<QueryRequest>,
    ) -> Result<Response<Self::queryStream>, Status> {
        let request = request.into_inner();
        let Some(query) = request.query else {
            return Err(Status::invalid_argument("missing query params"));
        };
        let rsvps = self.manager.query(query).await;
        let stream = TonicReceiverStream::new(rsvps);
        Ok(Response::new(Box::pin(stream)))
    }
    /// filter reservations, order by reservatioin id
    async fn filter(
        &self,
        request: Request<FilterRequest>,
    ) -> Result<Response<FilterResponse>, Status> {
        let request = request.into_inner();
        let Some(filter) = request.filter else {
            return Err(Status::invalid_argument("missing filter params"));
        };
        let (pager, reservations) = self.manager.filter(filter).await?;
        Ok(Response::new(FilterResponse {
            reservations,
            pager: Some(pager),
        }))
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

impl<T> TonicReceiverStream<T> {
    pub fn new(inner: mpsc::Receiver<Result<T, abi::Error>>) -> Self {
        Self { inner }
    }
}

impl<T> Stream for TonicReceiverStream<T> {
    type Item = Result<T, Status>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.inner.poll_recv(cx) {
            Poll::Ready(Some(Ok(item))) => Poll::Ready(Some(Ok(item))),
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e.into()))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
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
