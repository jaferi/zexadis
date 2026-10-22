//! Zexadis server binary entry point.

fn main() -> std::io::Result<()> {
    zexadis_server::tcp::run("127.0.0.1:6379")
}