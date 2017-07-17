extern crate aurora_client;

fn main() {
    aurora_client::run_service().expect("Service encountered an unhandled error");
}
