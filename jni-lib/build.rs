#![feature(catch_expr)]

use std::process::{exit, Command};

fn main() {
    let res: std::io::Result<()> = do catch {
        let _ = std::fs::DirBuilder::new().recursive(true).create("./dist")?;

        let status = Command::new("javac")
            .arg("-d")
            .arg("./dist")
            .arg("-sourcepath")
            .arg("./src/java")
            .arg("./src/java/uk/me/rochester/Aurora.java")
            .status()?;

        if !status.success() {
            eprintln!("javac failed");
            exit(1);
        }

        //let status = Command::new("javah")
        //    .arg("-cp")
        //    .arg("./dist")
        //    .arg("-o")
        //    .arg("jni_header.h")
        //    .arg("uk.me.rochester.Aurora")
        //    .status()?;

        //if !status.success() {
        //    eprintln!("javah failed");
        //    exit(1);
        //}

        Ok(())
    };
    res.expect("Build failed");
}