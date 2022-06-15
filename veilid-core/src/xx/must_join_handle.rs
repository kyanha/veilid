use async_executors::JoinHandle;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

#[derive(Debug)]
pub struct MustJoinHandle<T> {
    join_handle: JoinHandle<T>,
    completed: bool,
}

impl<T> MustJoinHandle<T> {
    pub fn new(join_handle: JoinHandle<T>) -> Self {
        Self {
            join_handle,
            completed: false,
        }
    }
}

impl<T> Drop for MustJoinHandle<T> {
    fn drop(&mut self) {
        // panic if we haven't completed
        if !self.completed {
            panic!("MustJoinHandle was not completed upon drop. Add cooperative cancellation where appropriate to ensure this is completed before drop.")
        }
    }
}

impl<T: 'static> Future for MustJoinHandle<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.join_handle).poll(cx) {
            Poll::Ready(t) => {
                self.completed = true;
                Poll::Ready(t)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
