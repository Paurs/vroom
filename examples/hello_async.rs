use std::error::Error;
use std::{env, process};

use vroom::driver::Driver;

#[tokio::main(flavor = "multi_thread")]
pub async fn main() -> Result<(), Box<dyn Error>> {
    //env::set_var("RUST_BACKTRACE", "1");

    let mut args = env::args();
    args.next();

    let pci_addr = match args.next() {
        Some(arg) => arg,
        None => {
            eprintln!("Usage: cargo run --example hello_world <pci bus id>");
            process::exit(1);
        }
    };

    let driver = Driver::new(&pci_addr, 4)?;

    let f1 = driver.read("Hello World".as_bytes(), 0);
    let f2 = driver.write("TEST".as_bytes(), 0);

    let _ = futures::future::join(f1, f2).await;

    Ok(())
}
