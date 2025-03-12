use futures::future::join_all;
use rand::Rng;
use std::error::Error;
use std::{env, process};

use vroom::memory::{Dma, DmaSlice};
use vroom::{HUGE_PAGE_SIZE, QUEUE_LENGTH};

#[tokio::main]
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

    let mut driver = vroom::init(&pci_addr)?;

    let mut pair1 = driver.nvme.create_io_queue_pair(QUEUE_LENGTH)?;

    let buffer: Dma<u8> = Dma::allocate(HUGE_PAGE_SIZE).unwrap();
    let blocks = 8;
    let bytes = 512 * blocks as usize;
    let ns_blocks = driver.nvme.namespaces.get(&1).unwrap().blocks / blocks;
    let mut rng = rand::thread_rng();
    let range = (0, ns_blocks);
    let lba = rng.gen_range(range.0..range.1);

    let requests = pair1.submit_async(&buffer.slice((0)..(1 * bytes)), lba, true);

    join_all(requests).await;

    Ok(())
}
