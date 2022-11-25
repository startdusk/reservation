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

    use abi::{DbConfig, Reservation, ServerConfig};
    use helper::*;

    #[tokio::test]
    async fn rpc_reserve_should_work() {
        let test_app = TestApp::new().await.unwrap();
        let config = Config {
            db: DbConfig {
                host: test_app.host.clone(),
                port: test_app.port.clone(),
                user: test_app.user.clone(),
                password: test_app.password.clone(),
                dbname: test_app.dbname.clone(),
                max_connections: 5,
            },
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 50001,
            },
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

    mod helper {
        use docker_tester::{start_container, stop_container};
        use sqlx::{Connection, Executor, PgConnection, PgPool};
        use std::{rc::Rc, thread, time};
        use uuid::Uuid;

        pub struct TestApp {
            pub host: String,
            pub port: u16,
            pub user: String,
            pub password: String,
            pub dbname: String,
            pub container_id: String,
        }

        impl TestApp {
            pub async fn new() -> Result<Self, anyhow::Error> {
                // config databse
                let dbname = Rc::new(Uuid::new_v4().to_string());
                let image = "postgres:14-alpine";
                let port = "5432";
                let args = &[
                    "-e",
                    "POSTGRES_USER=postgres",
                    "-e",
                    "POSTGRES_PASSWORD=password",
                ];
                let container =
                    start_container(image, port, args).expect("Failed to start Postgres container");
                let test_app = Self {
                    dbname: dbname.clone().to_string(),
                    container_id: container.id,
                    host: container.host,
                    port: container.port,
                    user: "postgres".to_string(),
                    password: "password".to_string(),
                };
                for i in 1..=10 {
                    match PgConnection::connect(&test_app.server_url()).await {
                        Ok(conn) => {
                            conn.close().await?;
                            println!("Postgres is ready to go");
                            break;
                        }
                        Err(err) => {
                            if i == 10 {
                                return Err(anyhow::anyhow!(err));
                            }
                            println!("Postgres is not ready");
                            let ten_millis = time::Duration::from_secs(i);
                            thread::sleep(ten_millis);
                        }
                    }
                }
                let mut conn = PgConnection::connect(&test_app.server_url())
                    .await
                    .expect("Cannot connect to Postgres");

                conn.execute(format!(r#"CREATE DATABASE "{}";"#, dbname.clone()).as_str())
                    .await
                    .expect("Failed to create database");

                // Migrate database
                let db_pool = PgPool::connect(&test_app.url())
                    .await
                    .expect("Failed to connect to Postgres with db");

                sqlx::migrate!("../migrations")
                    .run(&db_pool)
                    .await
                    .expect("Failed to migrate the database");

                db_pool.close().await;

                Ok(test_app)
            }

            pub fn server_url(&self) -> String {
                if self.password.is_empty() {
                    format!("postgres://{}@{}:{}", self.user, self.host, self.port)
                } else {
                    format!(
                        "postgres://{}:{}@{}:{}",
                        self.user, self.password, self.host, self.port
                    )
                }
            }
            pub fn url(&self) -> String {
                format!("{}/{}", self.server_url(), self.dbname)
            }
        }

        impl Drop for TestApp {
            fn drop(&mut self) {
                stop_container(self.container_id.clone())
                    .expect("Failed to stop Postgres container");
            }
        }
    }
}
