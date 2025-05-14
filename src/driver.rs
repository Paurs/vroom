use std::{collections::HashMap, error::Error, fmt::Debug, sync::Arc};

use futures::lock::Mutex;
use tokio::sync::oneshot::{self, Sender};

use crate::{
    cmd::NvmeCommand, memory::DmaSlice, pci::*, request::Request, NvmeDevice, NvmeQueuePair,
    QUEUE_LENGTH,
};

#[derive(Debug)]
pub struct Driver<T: DmaSlice + Debug> {
    queue_pairs: Vec<Mutex<NvmeQueuePair<T>>>,
    pending: Vec<Mutex<HashMap<u16, Sender<std::io::Result<()>>>>>,
    nvme: Arc<Mutex<NvmeDevice<T>>>,
}

#[allow(unreachable_code)]
impl<T: DmaSlice + std::marker::Sync + std::marker::Send + 'static + Debug> Driver<T> {
    #[tracing::instrument]
    pub fn new(pci_addr: &str, num_q_pairs: usize) -> Result<Arc<Self>, Box<dyn Error>> {
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

        let mut nvme = NvmeDevice::<T>::init(pci_addr)?;
        nvme.identify_controller()?;
        let ns = nvme.identify_namespace_list(0);
        for n in ns {
            //println!("ns_id: {n}");
            nvme.identify_namespace(n);
        }

        let mut queue_pairs = Vec::new();
        for _ in 0..num_q_pairs {
            queue_pairs.push(Mutex::new(nvme.create_io_queue_pair(QUEUE_LENGTH)?));
        }

        let mut pending = Vec::new();
        for _ in 0..num_q_pairs {
            pending.push(Mutex::new(HashMap::new()));
        }

        let driver = Arc::new(Driver {
            queue_pairs,
            pending,
            nvme: Arc::new(Mutex::new(nvme)),
        });

        driver.start_polling();

        Ok(driver)
    }

    #[tracing::instrument]
    async fn submit(
        &self,
        q_id: usize,
        data: &T,
        lba: u64,
        write: bool,
    ) -> Option<(Option<usize>, Vec<u16>)> {
        if let Some(mut q_pair) = self.queue_pairs[q_id].try_lock() {
            let (tail, ids) = q_pair.submit_async(data, lba, write);
            if !ids.is_empty() {
                return Some((tail, ids));
            }
        }
        None
    }

    #[tracing::instrument]
    #[allow(unused_assignments)]
    fn start_polling(self: &Arc<Self>) {
        for q_id in 0..self.queue_pairs.len() {
            let driver = Arc::clone(self);

            tokio::spawn(async move {
                let mut empty_poll_count = 0;
                loop {
                    let completed_ids = {
                        let mut q_pair = driver.queue_pairs[q_id].lock().await;
                        q_pair.poll_multi(16)
                    };

                    if !completed_ids.is_empty() {
                        empty_poll_count = 0;

                        for id in completed_ids {
                            if let Some(sender) = driver.pending[q_id].lock().await.remove(&id) {
                                let _ = sender.send(Ok(()));
                            }
                        }
                    } else {
                        empty_poll_count = std::cmp::min(empty_poll_count + 1, 20);

                        if empty_poll_count > 10 {
                            let sleep_duration =
                                std::time::Duration::from_micros(1 << (empty_poll_count - 10));
                            tokio::time::sleep(sleep_duration).await;
                        } else {
                            tokio::task::yield_now().await;
                        }
                    }
                }
            });
        }
    }

    #[tracing::instrument]
    pub async fn read(&self, q_id: usize, data: &T, lba: u64) -> Vec<Request> {
        let mut requests = Vec::new();
        let mut acutal_qid = q_id;
        loop {
            match self.submit(acutal_qid, data, lba, false).await {
                Some((tail, ids)) => {
                    if ids.is_empty() {
                        println!("Empty command id list");
                    }
                    for &c_id in ids.iter() {
                        let (sender, receiver) = oneshot::channel();
                        {
                            let mut pending = self.pending[q_id].lock().await;
                            pending.insert(c_id, sender);
                        }
                        requests.push(Request {
                            id: c_id,
                            receiver,
                            state: crate::request::State::Submitted,
                        });
                    }
                    if let Some(tail) = tail {
                        self.queue_pairs[acutal_qid]
                            .lock()
                            .await
                            .set_tail(tail as u32);
                    }
                    break;
                }
                None => acutal_qid = (acutal_qid + 1) % self.queue_pairs.len(),
            }
        }
        requests
    }

    #[tracing::instrument]
    pub async fn read_batch(&self, q_id: usize, datas: &[T], lbas: &[u64]) -> Vec<Request> {
        assert_eq!(
            datas.len(),
            lbas.len(),
            "data and lba have different lenght"
        );

        let mut requests = Vec::with_capacity(datas.len());
        let mut all_ids = Vec::with_capacity(datas.len());

        let mut q_pair = self.queue_pairs[q_id].lock().await;

        let mut last_tail = None;
        for (data, &lba) in datas.iter().zip(lbas.iter()) {
            let (tail, ids) = q_pair.submit_async(data, lba, false);
            all_ids.extend(ids);
            if let Some(tail) = tail {
                last_tail = Some(tail);
            }
        }
        drop(q_pair);
        for c_id in all_ids {
            let (sender, receiver) = oneshot::channel();
            {
                let mut pending = self.pending[q_id].lock().await;
                pending.insert(c_id, sender);
            }
            requests.push(Request {
                id: c_id,
                receiver,
                state: crate::request::State::Submitted,
            });
        }
        if let Some(tail) = last_tail {
            self.queue_pairs[q_id].lock().await.set_tail(tail as u32);
        }
        requests
    }

    #[tracing::instrument]
    pub async fn write(&self, q_id: usize, data: &T, lba: u64) -> Vec<Request> {
        let mut requests = Vec::new();
        let mut acutal_qid = q_id;
        loop {
            match self.submit(acutal_qid, data, lba, true).await {
                Some((tail, ids)) => {
                    if ids.is_empty() {
                        println!("Empty command id list");
                    }
                    for &c_id in ids.iter() {
                        let (sender, receiver) = oneshot::channel();
                        {
                            let mut pending = self.pending[q_id].lock().await;
                            pending.insert(c_id, sender);
                        }
                        requests.push(Request {
                            id: c_id,
                            receiver,
                            state: crate::request::State::Submitted,
                        });
                    }
                    if let Some(tail) = tail {
                        self.queue_pairs[acutal_qid]
                            .lock()
                            .await
                            .set_tail(tail as u32);
                    }
                    break;
                }
                None => acutal_qid = (acutal_qid + 1) % self.queue_pairs.len(),
            }
        }
        requests
    }

    #[tracing::instrument]
    pub async fn write_batch(&self, q_id: usize, datas: &[T], lbas: &[u64]) -> Vec<Request> {
        assert_eq!(
            datas.len(),
            lbas.len(),
            "data and lba have different lenght"
        );

        let mut requests = Vec::with_capacity(datas.len());
        let mut all_ids = Vec::with_capacity(datas.len());

        let mut q_pair = self.queue_pairs[q_id].lock().await;

        let mut last_tail = None;
        for (data, &lba) in datas.iter().zip(lbas.iter()) {
            let (tail, ids) = q_pair.submit_async(data, lba, true);
            all_ids.extend(ids);
            if let Some(tail) = tail {
                last_tail = Some(tail);
            }
        }
        drop(q_pair);
        for c_id in all_ids {
            let (sender, receiver) = oneshot::channel();
            {
                let mut pending = self.pending[q_id].lock().await;
                pending.insert(c_id, sender);
            }
            requests.push(Request {
                id: c_id,
                receiver,
                state: crate::request::State::Submitted,
            });
        }
        if let Some(tail) = last_tail {
            self.queue_pairs[q_id].lock().await.set_tail(tail as u32);
        }
        requests
    }

    #[tracing::instrument]
    pub async fn cleanup(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut nvme = self.nvme.lock().await;
        for q_pair in &self.queue_pairs {
            let id = q_pair.lock().await.id;

            nvme.submit_and_complete_admin(|c_id, _| {
                NvmeCommand::delete_io_submission_queue(c_id, id)
            })?;
            nvme.submit_and_complete_admin(|c_id, _| {
                NvmeCommand::delete_io_completion_queue(c_id, id)
            })?;
        }
        Ok(())
    }
}
