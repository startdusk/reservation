use std::{fs, path::Path, process::Command};

use tonic_build::Builder;

fn main() {
    let path = "src/pb";
    // Recursively create a directory and all of its parent components if they are missing
    fs::create_dir_all(path).unwrap();
    tonic_build::configure()
        .out_dir(path)
        .with_sql_type(&["reservation.ReservationStatus"])
        .with_builder(&["reservation.ReservationQuery"])
        .with_builder_into(
            "reservation.ReservationQuery",
            &[
                "user_id",
                "resource_id",
                "status",
                "page",
                "page_size",
                "desc",
            ],
        )
        .with_builder_option("reservation.ReservationQuery", &["start", "end"])
        .compile(&["protos/reservation.proto"], &["protos"])
        .unwrap();

    let exlude_file = Path::new("src/pb/google.protobuf.rs");
    if exlude_file.exists() {
        fs::remove_file(exlude_file).unwrap();
    }

    Command::new("cargo").args(&["fmt"]).output().unwrap();

    println!("cargo:rerun-if-changed=protos/reservation.proto");
}

trait BuilderExt {
    fn with_sql_type(self, paths: &[&str]) -> Self;
    fn with_builder(self, paths: &[&str]) -> Self;
    fn with_builder_into(self, path: &str, fields: &[&str]) -> Self;
    fn with_builder_option(self, path: &str, fields: &[&str]) -> Self;
}

impl BuilderExt for Builder {
    fn with_sql_type(self, paths: &[&str]) -> Self {
        paths.iter().fold(self, |acc, path| {
            acc.type_attribute(path, "#[derive(sqlx::Type)]")
        })
    }

    fn with_builder(self, paths: &[&str]) -> Self {
        paths.iter().fold(self, |acc, path| {
            acc.type_attribute(path, "#[derive(derive_builder::Builder)]")
        })
    }

    fn with_builder_into(self, path: &str, fields: &[&str]) -> Self {
        fields.iter().fold(self, |acc, field| {
            acc.field_attribute(
                &format!("{}.{}", path, field),
                "#[builder(setter(into), default)]",
            )
        })
    }

    fn with_builder_option(self, path: &str, fields: &[&str]) -> Self {
        fields.iter().fold(self, |acc, field| {
            acc.field_attribute(
                &format!("{}.{}", path, field),
                "#[builder(setter(into, strip_option))]",
            )
        })
    }
}
