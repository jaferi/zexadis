//! Zexadis server binary entry point.

use zexadis_server::tcp::start_server;

fn main() {
    println!("Hello from Zexadis!");
    start_server("0.0.0.0:2828").unwrap();
}
