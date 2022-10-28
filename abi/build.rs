use std::{fs, process::Command};

fn main() {
    let path = "src/pb";
    // Recursively create a directory and all of its parent components if they are missing
    fs::create_dir_all(path).unwrap();
    tonic_build::configure()
        .out_dir(path)
        .type_attribute("reservation.ReservationStatus", "#[derive(sqlx::Type)]")
        .compile(&["protos/reservation.proto"], &["protos"])
        .unwrap();

    fs::remove_file("src/pb/google.protobuf.rs").unwrap();

    Command::new("cargo").args(&["fmt"]).output().unwrap();

    println!("cargo:rerun-if-changed=protos/reservation.proto");
}
