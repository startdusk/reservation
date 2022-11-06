use std::{fs, path::Path, process::Command};

fn main() {
    let path = "src/pb";
    // Recursively create a directory and all of its parent components if they are missing
    fs::create_dir_all(path).unwrap();
    tonic_build::configure()
        .out_dir(path)
        .type_attribute("reservation.ReservationStatus", "#[derive(sqlx::Type)]")
        .compile(&["protos/reservation.proto"], &["protos"])
        .unwrap();

    let exlude_file = Path::new("src/pb/google.protobuf.rs");
    if exlude_file.exists() {
        fs::remove_file(exlude_file).unwrap();
    }

    Command::new("cargo").args(&["fmt"]).output().unwrap();

    println!("cargo:rerun-if-changed=protos/reservation.proto");
}
