use crate::dart_serialize::*;
pub use allo_isolate::ffi::DartCObject;
pub use allo_isolate::IntoDart;
use allo_isolate::Isolate;
use core::future::Future;
use parking_lot::Mutex;
use serde::*;
use std::sync::Arc;

#[derive(Clone)]
pub struct DartIsolateWrapper {
    isolate: Isolate,
}

const MESSAGE_OK: i32 = 0;
//const MESSAGE_ERR: i32 = 1;
const MESSAGE_OK_JSON: i32 = 2;
const MESSAGE_ERR_JSON: i32 = 3;
//const MESSAGE_STREAM_ITEM: i32 = 4;
const MESSAGE_STREAM_ITEM_JSON: i32 = 5;
//const MESSAGE_STREAM_ABORT: i32 = 6;
const MESSAGE_STREAM_ABORT_JSON: i32 = 7;
const MESSAGE_STREAM_CLOSE: i32 = 8;

impl DartIsolateWrapper {
    pub fn new(port: i64) -> Self {
        DartIsolateWrapper {
            isolate: Isolate::new(port),
        }
    }

    pub fn spawn_result<F, T, E>(self, future: F)
    where
        F: Future<Output = Result<T, E>> + Send + 'static,
        T: IntoDart,
        E: Serialize,
    {
        async_std::task::spawn(async move {
            self.result(future.await);
        });
    }

    pub fn spawn_result_json<F, T, E>(self, future: F)
    where
        F: Future<Output = Result<T, E>> + Send + 'static,
        T: Serialize,
        E: Serialize,
    {
        async_std::task::spawn(async move {
            self.result_json(future.await);
        });
    }

    pub fn result<T: IntoDart, E: Serialize>(&self, result: Result<T, E>) -> bool {
        match result {
            Ok(v) => self.ok(v),
            Err(e) => self.err_json(e),
        }
    }
    pub fn result_json<T: Serialize, E: Serialize>(&self, result: Result<T, E>) -> bool {
        match result {
            Ok(v) => self.ok_json(v),
            Err(e) => self.err_json(e),
        }
    }
    pub fn ok<T: IntoDart>(&self, value: T) -> bool {
        self.isolate
            .post(vec![MESSAGE_OK.into_dart(), value.into_dart()])
    }

    pub fn ok_json<T: Serialize>(&self, value: T) -> bool {
        self.isolate.post(vec![
            MESSAGE_OK_JSON.into_dart(),
            serialize_json(value).into_dart(),
        ])
    }

    // pub fn err<E: IntoDart>(&self, error: E) -> bool {
    //     self.isolate
    //         .post(vec![MESSAGE_ERR.into_dart(), error.into_dart()])
    // }

    pub fn err_json<E: Serialize>(&self, error: E) -> bool {
        self.isolate.post(vec![
            MESSAGE_ERR_JSON.into_dart(),
            serialize_json(error).into_dart(),
        ])
    }
}

#[derive(Clone)]
pub struct DartIsolateStream {
    isolate: Arc<Mutex<Option<Isolate>>>,
}

impl DartIsolateStream {
    pub fn new(port: i64) -> Self {
        DartIsolateStream {
            isolate: Arc::new(Mutex::new(Some(Isolate::new(port)))),
        }
    }

    // pub fn item<T: IntoDart>(&self, value: T) -> bool {
    //     let isolate = self.isolate.lock();
    //     if let Some(isolate) = &*isolate {
    //         isolate.post(vec![MESSAGE_STREAM_ITEM.into_dart(), value.into_dart()])
    //     } else {
    //         false
    //     }
    // }

    pub fn item_json<T: Serialize>(&self, value: T) -> bool {
        let isolate = self.isolate.lock();
        if let Some(isolate) = &*isolate {
            isolate.post(vec![
                MESSAGE_STREAM_ITEM_JSON.into_dart(),
                serialize_json(value).into_dart(),
            ])
        } else {
            false
        }
    }

    // pub fn abort<E: IntoDart>(self, error: E) -> bool {
    //     let mut isolate = self.isolate.lock();
    //     if let Some(isolate) = isolate.take() {
    //         isolate.post(vec![MESSAGE_STREAM_ABORT.into_dart(), error.into_dart()])
    //     } else {
    //         false
    //     }
    // }

    pub fn abort_json<E: Serialize>(self, error: E) -> bool {
        let mut isolate = self.isolate.lock();
        if let Some(isolate) = isolate.take() {
            isolate.post(vec![
                MESSAGE_STREAM_ABORT_JSON.into_dart(),
                serialize_json(error).into_dart(),
            ])
        } else {
            false
        }
    }

    pub fn close(self) -> bool {
        let mut isolate = self.isolate.lock();
        if let Some(isolate) = isolate.take() {
            isolate.post(vec![MESSAGE_STREAM_CLOSE.into_dart()])
        } else {
            false
        }
    }
}

impl Drop for DartIsolateStream {
    fn drop(&mut self) {
        let mut isolate = self.isolate.lock();
        if let Some(isolate) = isolate.take() {
            isolate.post(vec![MESSAGE_STREAM_CLOSE.into_dart()]);
        }
    }
}
