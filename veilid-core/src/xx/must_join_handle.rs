use super::*;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

#[derive(Debug)]
pub struct MustJoinHandle<T> {
    join_handle: Option<LowLevelJoinHandle<T>>,
    completed: bool,
}

impl<T> MustJoinHandle<T> {
    pub fn new(join_handle: LowLevelJoinHandle<T>) -> Self {
        Self {
            join_handle: Some(join_handle),
            completed: false,
        }
    }

    #[allow(unused_mut)]
    pub async fn abort(mut self) {
        if !self.completed {
            cfg_if! {
                if #[cfg(feature="rt-async-std")] {
                    if let Some(jh) = self.join_handle.take() {
                        jh.cancel().await;
                        self.completed = true;
                    }
                } else if #[cfg(feature="rt-tokio")] {
                    if let Some(jh) = self.join_handle.take() {
                        jh.abort();
                        let _ = jh.await;
                        self.completed = true;
                    }
                } else if #[cfg(target_arch = "wasm32")] {
                    drop(self.join_handle.take());
                    self.completed = true;
                } else {
                    compile_error!("needs executor implementation")
                }

            }
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
        match Pin::new(self.join_handle.as_mut().unwrap()).poll(cx) {
            Poll::Ready(t) => {
                self.completed = true;
                cfg_if! {
                    if #[cfg(feature="rt-async-std")] {
                        Poll::Ready(t)
                    } else if #[cfg(feature="rt-tokio")] {
                        Poll::Ready(t.unwrap())
                    }else if #[cfg(target_arch = "wasm32")] {
                        Poll::Ready(t)
                    } else {
                        compile_error!("needs executor implementation")
                    }
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
