use std::time::Duration;

use abi::{
    reservation_service_client::ReservationServiceClient, Config, ConfirmRequest, DbConfig,
    FilterRequest, FilterResponse, QueryRequest, Reservation, ReservationFilterBuilder,
    ReservationQueryBuilder, ReservationStatus, ReserveRequest, ServerConfig,
};
use docker_tester::TestPostgres;
use reservation_service::start_server;
use tokio::time;
use tokio_stream::StreamExt;
use tonic::transport::Channel;

#[tokio::test]
async fn grpc_server_should_work() {
    let test_app = TestPostgres::new("../migrations").await.unwrap();
    let mut client = get_test_client(&test_app, 50051).await;
    // first we make a reservation
    let mut rsvp = Reservation::new_pending(
        "user_id_1",
        "ocean-view-room-713",
        "2022-12-25T15:00:00-0700".parse().unwrap(),
        "2022-12-28T12:00:00-0700".parse().unwrap(),
        "hello I'm user 1.",
    );

    let ret = client
        .reserve(ReserveRequest::new(rsvp.clone()))
        .await
        .unwrap()
        .into_inner()
        .reservation
        .unwrap();
    let confirm_rsvp_id = ret.id;
    rsvp.id = ret.id;
    assert_eq!(ret, rsvp);

    // then we try to make a conflicting reservation
    let rsvp2 = Reservation::new_pending(
        "user_id_1",
        "ocean-view-room-713",
        "2022-12-25T15:00:00-0700".parse().unwrap(),
        "2022-12-28T12:00:00-0700".parse().unwrap(),
        "hello I'm user 1.",
    );

    let ret = client.reserve(ReserveRequest::new(rsvp2.clone())).await;
    assert!(ret.is_err());

    // then we confirm first reservation
    let ret = client
        .confirm(ConfirmRequest::new(confirm_rsvp_id))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        ret.reservation.unwrap().status,
        ReservationStatus::Confirmed as i32
    );
}

#[tokio::test]
async fn grpc_query_should_work() {
    let test_app = TestPostgres::new("../migrations").await.unwrap();
    let mut client = get_test_client(&test_app, 50052).await;
    make_reservations(&mut client, 100, "user").await;

    let query = ReservationQueryBuilder::default()
        .user_id("user")
        .build()
        .unwrap();
    // query all reservations
    let mut ret = client
        .query(QueryRequest::new(query))
        .await
        .unwrap()
        .into_inner();

    while let Some(Ok(rsvp)) = ret.next().await {
        assert_eq!(rsvp.user_id, "user");
    }
}

#[tokio::test]
async fn grpc_filter_should_work() {
    let test_app = TestPostgres::new("../migrations").await.unwrap();
    let mut client = get_test_client(&test_app, 50053).await;
    // then we make 100 reservations without confliction
    let filter_user_id = "filter_user_id";
    make_reservations(&mut client, 25, filter_user_id).await;
    // then we filter by user
    let filter = ReservationFilterBuilder::default()
        .user_id(filter_user_id)
        .status(ReservationStatus::Pending as i32)
        .build()
        .unwrap();

    let FilterResponse {
        pager,
        reservations,
    } = client
        .filter(FilterRequest::new(filter.clone()))
        .await
        .unwrap()
        .into_inner();

    let pager = pager.unwrap();
    // assert_eq!(pager.total, 100) // no implemented yet
    assert_eq!(pager.prev, None);
    assert_eq!(pager.next, Some(filter.page_size)); // we alreay had an item

    assert_eq!(reservations.len(), filter.page_size as usize);

    // then we get next page
    let filter = filter.next_page(&pager).unwrap();
    let FilterResponse {
        pager,
        reservations,
    } = client
        .filter(FilterRequest::new(filter.clone()))
        .await
        .unwrap()
        .into_inner();

    let pager = pager.unwrap();

    assert_eq!(pager.prev, filter.cursor.map(|v| v + 1));
    assert_eq!(pager.next, filter.cursor.map(|v| v + filter.page_size));

    assert_eq!(reservations.len(), filter.page_size as usize);

    // then we get next page (last page)
    let filter = filter.next_page(&pager).unwrap();
    let FilterResponse {
        pager,
        reservations,
    } = client
        .filter(FilterRequest::new(filter.clone()))
        .await
        .unwrap()
        .into_inner();

    let pager = pager.unwrap();

    assert_eq!(pager.prev, filter.cursor.map(|v| v + 1));
    assert_eq!(pager.next, None);

    assert_eq!(reservations.len(), 5);
}

async fn get_test_client(test_app: &TestPostgres, port: u16) -> ReservationServiceClient<Channel> {
    let config = Config {
        db: DbConfig {
            host: test_app.host.clone(),
            port: test_app.port,
            user: test_app.user.clone(),
            password: test_app.password.clone(),
            dbname: test_app.dbname.clone(),
            max_connections: 5,
        },
        server: ServerConfig {
            host: "0.0.0.0".into(),
            port,
        },
    };
    setup_server(&config);

    let fut = async move {
        // if error on conn keep retry until timeout
        while ReservationServiceClient::connect(config.server.url(false))
            .await
            .is_err()
        {
            time::sleep(Duration::from_millis(10)).await
        }
        ReservationServiceClient::connect(config.server.url(false))
            .await
            .unwrap()
    };

    time::timeout(Duration::from_secs(5), fut).await.unwrap()
}

fn setup_server(config: &Config) {
    let config_cloned = config.clone();
    tokio::spawn(async move {
        start_server(&config_cloned).await.unwrap();
    });
}

async fn make_reservations(
    client: &mut ReservationServiceClient<Channel>,
    count: usize,
    uid: &str,
) {
    for i in 0..count {
        let mut rsvp = Reservation::new_pending(
            uid,
            format!("router-{i}"),
            "2022-12-25T15:00:00-0700".parse().unwrap(),
            "2022-12-28T12:00:00-0700".parse().unwrap(),
            format!("test device reservation {i}"),
        );

        let ret = client
            .reserve(ReserveRequest::new(rsvp.clone()))
            .await
            .unwrap()
            .into_inner()
            .reservation
            .unwrap();
        rsvp.id = ret.id;
        assert_eq!(ret, rsvp);
    }
}
