use std::error::Error;
use std::{env, process};

use vroom::driver::Driver;
use vroom::memory::{Dma, DmaSlice};
use vroom::HUGE_PAGE_SIZE;

#[tokio::main(flavor = "multi_thread")]
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

    let driver = Driver::<Dma<u8>>::new(&pci_addr, 4)?;

    let bytes = 8 * 512;
    let rand_block = &(0..HUGE_PAGE_SIZE)
        .map(|_| rand::random::<u8>())
        .collect::<Vec<_>>()[..];
    let mut buffer: Dma<u8> = Dma::allocate(HUGE_PAGE_SIZE).unwrap();
    buffer[0..HUGE_PAGE_SIZE].copy_from_slice(rand_block);

    buffer[0..12].copy_from_slice("Hello World!".as_bytes());

    let f1 = driver.write(0, &buffer.slice(0..bytes), 0).await;
    let _ = futures::future::join_all(f1).await;
    let f2 = driver.read(0, &buffer.slice(0..bytes), 0).await;
    let _ = futures::future::join_all(f2).await;

    for b in buffer.chunks(2 * 4096) {
        for byte in b.slice.iter().take(12) {
            if let Some(char) = std::char::from_u32(*byte as u32) {
                print!("{}", char);
            }
        }
        println!("");
        break;
    }

    Ok(())
}
