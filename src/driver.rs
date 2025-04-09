use std::collections::HashMap;
use std::time::Duration;
use std::{error::Error, sync::Arc};

use futures::{channel::mpsc, SinkExt};
use tokio::sync::oneshot;
use tokio::sync::Mutex;

use crate::memory::DmaSlice;
use crate::{
    nvme_future::{Request, State},
    pci::*,
    NvmeDevice, QUEUE_LENGTH,
};

pub struct IoRequest<'a, T: DmaSlice + 'a> {
    sender: oneshot::Sender<()>,
    data: &'a T,
    lba: u64,
    write: bool,
}

struct InternalState<'a, T: DmaSlice + 'a> {
    senders: Vec<mpsc::Sender<IoRequest<'a, T>>>,
    num_q_pairs: usize,
}

pub struct Driver<'a, T: DmaSlice + 'a> {
    internal: Arc<Mutex<InternalState<'a, T>>>,
}

impl<'a, T: DmaSlice + std::marker::Sync + std::marker::Send> Driver<'a, T> {
    pub fn new(pci_addr: &str, num_q_pairs: usize) -> Result<Self, Box<dyn Error>> {
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

        let mut senders = Vec::new();
        let mut handles = vec![];
        let nvme_arc = Arc::new(Mutex::new(nvme));

        for _ in 0..num_q_pairs {
            let nvme_clone = nvme_arc.clone();

            let (tx, mut rx) = mpsc::channel::<IoRequest<T>>(32);
            senders.push(tx);

            let handle = tokio::task::spawn(async move {
                let mut q_pair = nvme_clone
                    .lock()
                    .await
                    .create_io_queue_pair(QUEUE_LENGTH)
                    .unwrap();

                let mut requests: Vec<Request> = Vec::new();
                let mut next_request_id: usize = 0;
                let mut response_senders: HashMap<usize, oneshot::Sender<()>> = HashMap::new();

                loop {
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    // poll the completion queue and send inform calling awaiting function
                    while let Some(completion) = q_pair.poll() {
                        let c_id = completion.c_id;

                        if let Some(index) = requests.iter().position(|r| r.c_id == c_id) {
                            let mut req = requests.remove(index);
                            req.state = State::Completed(completion);
                            q_pair
                                .outstanding
                                .entry(req.r_id)
                                .and_modify(|num| *num -= 1);

                            if let Some(&value) = q_pair.outstanding.get(&req.r_id) {
                                if value == 0 {
                                    q_pair.outstanding.remove(&req.r_id);
                                    let _ = response_senders.remove(&req.r_id).unwrap().send(());
                                }
                            }

                            println!("{:?}", completion);
                        }
                    }

                    // wait for I/O requests and submit them to submission queue
                    while let Ok(Some(req)) = rx.try_next() {
                        let mut new_ftrs =
                            q_pair.submit_async(req.data, req.lba, req.write, next_request_id);
                        requests.append(&mut new_ftrs);
                        response_senders.insert(next_request_id, req.sender);
                        next_request_id += 1;
                    }

                    println!("{:?}", std::thread::current().id());
                }
            });

            handles.push(handle);
        }

        let _ = futures::future::join_all(handles);

        Ok(Driver {
            internal: Arc::new(Mutex::new(InternalState {
                senders,
                num_q_pairs,
            })),
        })
    }

    pub async fn read(&self, dest: &'a T, lba: u64) -> Result<(), Box<dyn Error>> {
        let mut internal_state = self.internal.lock().await;

        // oneshoot channel to recevie a response when I/O request has been completed
        let (response_tx, response_rx) = oneshot::channel();

        // Choose a Queue pair to submit I/O request
        let q_id = (lba % internal_state.num_q_pairs as u64) as usize;

        let request = IoRequest {
            sender: response_tx,
            data: dest,
            lba: lba,
            write: false,
        };

        // Send the I/O request to the tokio task managing the chosen queue pair
        if let Some(sender) = internal_state.senders.get_mut(q_id) {
            sender.send(request).await.unwrap();
        } else {
            return Err("Invalid queue id".into());
        }

        Ok(response_rx.await.unwrap())
    }

    pub async fn write(&self, data: &'a T, lba: u64) -> Result<(), Box<dyn Error>> {
        let mut internal_state = self.internal.lock().await;

        // oneshoot channel to recevie a response when I/O request has been completed
        let (response_tx, response_rx) = oneshot::channel();

        // Choose a Queue pair to submit I/O request
        let q_id = (lba % internal_state.num_q_pairs as u64) as usize;

        let request = IoRequest {
            sender: response_tx,
            data: data,
            lba: lba,
            write: true,
        };

        // Send the I/O request to the tokio task managing the chosen queue pair
        if let Some(sender) = internal_state.senders.get_mut(q_id) {
            sender.send(request).await.unwrap();
        } else {
            return Err("Invalid queue id".into());
        }

        Ok(response_rx.await.unwrap())
    }
}
