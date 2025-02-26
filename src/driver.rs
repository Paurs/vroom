use std::error::Error;

use crate::{pci::*, NvmeDevice, NvmeQueuePair, QUEUE_LENGTH};

pub struct Driver {
    pub nvme: NvmeDevice,
    pub pairs: Vec<NvmeQueuePair>,
}

impl Driver {
    pub fn new(pci_addr: &str, queue_num: usize) -> Result<Self, Box<dyn Error>> {
        let mut vendor_file = pci_open_resource_ro(pci_addr, "vendor").expect("wrong pci address");
        let mut device_file = pci_open_resource_ro(pci_addr, "device").expect("wrong pci address");
        let mut config_file = pci_open_resource_ro(pci_addr, "config").expect("wrong pci address");

        let _vendor_id = read_hex(&mut vendor_file)?;
        let _device_id = read_hex(&mut device_file)?;
        let class_id = read_io32(&mut config_file, 8)? >> 16;

        // 0x01 -> mass storage device class id
        // 0x08 -> nvme subclass
        if class_id != 0x0108 {
            return Err(format!("device {} is not a block device", pci_addr).into());
        }

        let mut nvme = NvmeDevice::init(pci_addr)?;
        nvme.identify_controller()?;
        let ns = nvme.identify_namespace_list(0);
        for n in ns {
            println!("ns_id: {n}");
            nvme.identify_namespace(n);
        }

        let mut pairs = Vec::new();
        for _ in 0..queue_num {
            let qp = nvme.create_io_queue_pair(QUEUE_LENGTH)?;
            pairs.push(qp);
        }

        Ok(Driver {
            nvme: nvme,
            pairs: pairs,
        })
    }

    pub async fn listen(&mut self) {
        loop {
            self.poll_queue(1).await;
        }
    }

    pub async fn poll_queue(&mut self, q_id: u16) {
        if let Some(pair) = self
            .pairs
            .iter_mut()
            .find(|&&mut NvmeQueuePair { id, .. }| id == q_id)
        {
            pair.poll().await;
        } else {
            println!("not found");
        }
    }

    pub async fn poll(&mut self) {
        for p in self.pairs.iter_mut() {
            p.poll().await;
        }
    }
}
