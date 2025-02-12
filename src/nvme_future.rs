use crate::queues::{NvmeCompQueue, NvmeCompletion};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

pub enum State {
    Submitted,
    Waiting(Waker),
    Completed(NvmeCompletion),
}

pub struct NvmeFuture<'a> {
    io_cq: &'a mut NvmeCompQueue,
    c_id: u16,
    head: usize,
}

impl<'a> NvmeFuture<'a> {
    pub fn new(io_cq: &'a mut NvmeCompQueue, c_id: u16) -> Self {
        unsafe {
            let head = std::ptr::read_unaligned(io_cq.doorbell as *mut u32) as usize;
            Self { io_cq, c_id, head }
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
                        std::ptr::write_volatile(this.io_cq.doorbell as *mut u32, head as u32);
                    }
                    return Poll::Ready((head, val, prev));
                }
                Poll::Pending
            }
        }
    }
}

pub struct Request {
    pub state: State,
}

impl Future for Request {
    type Output = NvmeCompletion;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        println!("polling... ");

        match self.state {
            State::Submitted => {
                self.state = State::Waiting(cx.waker().clone());
                Poll::Pending
            }
            State::Waiting(_) => {
                self.state = State::Waiting(cx.waker().clone());
                Poll::Pending
            }
            State::Completed(completion) => Poll::Ready(completion.clone()),
        }
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        match self.state {
            State::Completed(_) => {}
            _ => panic!("Request dropped before completion."),
        };
    }
}
