use rand::Rng;
use std::error::Error;
use std::time::Duration;
use std::{env, process};

use tokio::time::sleep;
use vroom::memory::{Dma, DmaSlice};
use vroom::HUGE_PAGE_SIZE;

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

    let mut driver = vroom::init(&pci_addr, 1);

    let buffer: Dma<u8> = Dma::allocate(HUGE_PAGE_SIZE).unwrap();
    let ctr = 0;
    let blocks = 8;
    let bytes = 512 * blocks as usize;
    let ns_blocks = driver.nvme.namespaces.get(&1).unwrap().blocks / blocks;
    let mut rng = rand::thread_rng();
    let range = (0, ns_blocks);
    let lba = rng.gen_range(range.0..range.1);

    let _ = driver.pairs[0].submit_async(
        &buffer.slice((ctr * bytes)..(ctr + 1) * bytes),
        lba * blocks,
        true,
    );

    sleep(Duration::from_secs(1)).await;

    driver.pairs[0].poll().await;

    Ok(())
}
