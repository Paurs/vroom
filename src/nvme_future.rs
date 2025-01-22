use crate::queues::{NvmeCompQueue, NvmeCompletion};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct NvmeFuture<'a> {
    io_cq: &'a mut NvmeCompQueue,
    c_id: u16,
    q_id: u16,
    addr: *mut u8,
    dstrd: u16,
}

impl<'a> NvmeFuture<'a> {
    pub fn new(
        io_cq: &'a mut NvmeCompQueue,
        c_id: u16,
        q_id: u16,
        addr: *mut u8,
        dstrd: u16,
    ) -> Self {
        Self {
            io_cq,
            c_id,
            q_id,
            addr,
            dstrd,
        }
    }
}

impl Future for NvmeFuture<'_> {
    type Output = (usize, NvmeCompletion, usize);

    fn poll(self: Pin<&mut Self>, _context: &mut Context) -> Poll<(usize, NvmeCompletion, usize)> {
        let mut this = self;
        match this.io_cq.complete_async() {
            None => {
                _context.waker().wake_by_ref();
                Poll::Pending
            }
            Some(val) => {
                if this.c_id == val.c_id {
                    let (head, prev) = this.io_cq.new_head();

                    unsafe {
                        //std::ptr::write_volatile(this.io_cq.doorbell as *mut u32, head as u32);
                        std::ptr::write_volatile(
                            (this.addr as usize
                                + 0x1000
                                + (4 << this.dstrd) * (2 * this.q_id) as usize)
                                as *mut u32,
                            head as u32,
                        );
                    }
                    return Poll::Ready((head, val, prev));
                }
                Poll::Pending
            }
        }
    }
}
