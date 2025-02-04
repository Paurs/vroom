use std::error::Error;
use std::{env, process};

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args();
    args.next();

    let pci_addr = match args.next() {
        Some(arg) => arg,
        None => {
            eprintln!("Usage: cargo run --example hello_world <pci bus id>");
            process::exit(1);
        }
    };

    let mut driver = vroom::init(&pci_addr);
    driver
        .nvme
        .write_copied_async("hello world!".as_bytes(), 0)
        .await;

    let mut dest = [0u8; 12];
    driver.nvme.read_copied_async(&mut dest, 0).await;

    println!("{}", std::str::from_utf8(&dest)?);

    Ok(())
}
