// Copyright (c) Microsoft. All rights reserved.

use libc::uid_t;

pub struct UidService<T> {
    uid: uid_t,
    inner: T,
}

impl<T> UidService<T> {
    pub fn new(uid: uid_t, inner: T) -> Self {
        UidService { uid, inner }
    }
}

impl<T> hyper::service::Service<hyper::Request<hyper::Body>> for UidService<T>
where
    T: hyper::service::Service<
        hyper::Request<hyper::Body>,
        Response = hyper::Response<hyper::Body>,
        Error = std::convert::Infallible,
    >,
    <T as hyper::service::Service<hyper::Request<hyper::Body>>>::Future: Send + 'static,
{
    type Response = T::Response;
    type Error = std::convert::Infallible;
    type Future = std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<hyper::Response<hyper::Body>, std::convert::Infallible>,
                > + Send,
        >,
    >;

    fn call(&mut self, req: hyper::Request<hyper::Body>) -> Self::Future {
        let mut req = req;
        req.extensions_mut().insert(self.uid);
        Box::pin(self.inner.call(req))
    }

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }
}
