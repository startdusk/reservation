use sqlx::postgres::PgListener;

/// cargo run --example listen
/// and insert data to rsvp.reservations
/// like: insert into rsvp.reservations(user_id, resource_id, timespan) VALUES ('alice', 'room-443', '("2023-02-10", "2023-02-17")');

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let mut listener = PgListener::connect(&url).await.unwrap();
    listener.listen("reservation_update").await.unwrap();
    println!("Listening for reservation_update events...");
    loop {
        let notification = listener.recv().await.unwrap();
        println!("Received notification: {:?}", notification)
    }
}
