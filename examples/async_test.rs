use std::error::Error;
use std::time::{Duration, Instant};
use std::{env, process};

use vroom::driver::Driver;
use vroom::memory::{Dma, DmaSlice};
use vroom::HUGE_PAGE_SIZE;

#[tokio::main(flavor = "multi_thread")]
pub async fn main() -> Result<(), Box<dyn Error>> {
    //env::set_var("RUST_BACKTRACE", "1");

    let mut args = env::args();
    args.next();

    let pci_addr = match args.next() {
        Some(arg) => arg,
        None => {
            eprintln!("Usage: cargo run --example init <pci bus id> <number of queue pairs> <duration in seconds>");
            process::exit(1);
        }
    };

    let queue_num = match args.next() {
        Some(arg) => usize::from_str_radix(&arg, 10)?,
        None => {
            eprintln!("Usage: cargo run --example init <pci bus id> <number of queue pairs> <duration in seconds>");
            process::exit(1);
        }
    };

    let duration = match args.next() {
        Some(secs) => Some(Duration::from_secs(secs.parse().expect(
            "Usage: cargo run --example init <pci bus id> <number of queue pairs> <duration in seconds>",
        ))),
        None => None,
    };

    let mut driver = Driver::<Dma<u8>>::new(&pci_addr, queue_num)?;

    let time = duration.unwrap();

    let bytes = 512 * 8;

    let buffer: Dma<u8> = Dma::allocate(HUGE_PAGE_SIZE).unwrap();

    let start = Instant::now();

    let mut op_count: usize = 0;

    let mut handles = Vec::new();

    while start.elapsed() < time {
        for i in 0..queue_num {
            let handle = driver.read(buffer.slice(0..bytes), i as u64)?;
            handles.push(handle);
        }

        op_count += queue_num;

        if (op_count % (queue_num * (256))) == 0 {
            for handle in handles.drain(..) {
                let _ = handle.await;
            }
        }
    }

    futures::future::join_all(handles).await;

    println!("Time up, ops: {}", op_count);

    println!("{} iops", (op_count as f64 / time.as_secs() as f64));

    driver.cleanup().await?;

    Ok(())
}
