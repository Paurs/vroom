#![cfg_attr(target_arch = "aarch64", feature(stdarch_arm_hints))]
#[allow(unused)]
mod cmd;
#[allow(dead_code)]
pub mod driver;
#[allow(dead_code)]
mod handle;
#[allow(dead_code)]
pub mod memory;
#[allow(dead_code)]
mod nvme;
#[allow(dead_code)]
pub mod nvme_future;
#[allow(dead_code)]
mod pci;
#[allow(dead_code)]
mod queues;

use driver::Driver;
pub use memory::HUGE_PAGE_SIZE;
pub use nvme::{NvmeDevice, NvmeQueuePair};
pub use queues::QUEUE_LENGTH;

pub fn init(pci_addr: &str) -> Driver {
    Driver::new(pci_addr).unwrap()
}

#[derive(Debug, Clone, Copy)]
pub struct NvmeNamespace {
    pub id: u32,
    pub blocks: u64,
    pub block_size: u64,
}

#[derive(Debug, Clone, Default)]
pub struct NvmeStats {
    pub completions: u64,
    pub submissions: u64,
}
