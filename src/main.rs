// Copyright 3Bian Limited and Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]

mod cli;

fn main() {
    match cli::execute_cli() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("cheriot-compose: {error}");
            std::process::exit(2);
        }
    }
}
