use std::fmt::Display;
use std::task::Poll;
use std::{future::Future, pin::Pin};

use tokio::sync::oneshot;

#[derive(Debug)]
pub enum State {
    Submitted,
    Pending,
    Completed,
    Error,
}

impl Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Submitted => write!(f, "Submitted"),
            Self::Pending => write!(f, "Waiting"),
            Self::Completed => write!(f, "Completed"),
            Self::Error => write!(f, "Error"),
        }
    }
}

#[derive(Debug)]
pub struct Request {
    pub id: u16,
    pub receiver: oneshot::Receiver<std::io::Result<()>>,
    pub state: State,
}

impl Request {}

impl Future for Request {
    type Output = std::io::Result<()>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match Pin::new(&mut self.receiver).poll(cx) {
            Poll::Ready(Ok(result)) => {
                self.state = State::Completed;
                Poll::Ready(result)
            }
            Poll::Ready(Err(_)) => {
                self.state = State::Error;
                Poll::Ready(Err(std::io::Error::other(
                    "NVMe command completion channel closed unexpectedly.",
                )))
            }
            Poll::Pending => {
                self.state = State::Pending;
                Poll::Pending
            }
        }
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        match self.state {
            State::Completed => {}
            _ => panic!(
                "Request dropped before completion with state {}",
                self.state
            ),
        }
    }
}
