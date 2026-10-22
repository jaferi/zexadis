//! Zexadis server binary entry point.

fn main() -> std::io::Result<()> {
    println!("Zexadis Server v{}", env!("CARGO_PKG_VERSION"));
    println!("High-performance binary state store");
    println!();

    zexadis_server::tcp::run("127.0.0.1:6379")
}