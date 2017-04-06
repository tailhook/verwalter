use std::sync::Arc;

use futures::Async;
use tk_http::server::{Error, Codec, RecvMode, Encoder};
use tk_http::server as http;

use frontend::{Request, Reply};


pub struct QuickReply<F> {
    inner: Option<F>,
}


pub fn reply<F, S: 'static>(f: F)
    -> Request<S>
    where F: FnOnce(Encoder<S>) -> Reply<S> + 'static,
{
    Box::new(QuickReply {
        inner: Some(f),
    })
}

impl<F, S> Codec<S> for QuickReply<F>
    where F: FnOnce(Encoder<S>) -> Reply<S>,
{
    type ResponseFuture = Reply<S>;
    fn recv_mode(&mut self) -> RecvMode {
        RecvMode::buffered_upfront(0)
    }
    fn data_received(&mut self, data: &[u8], end: bool)
        -> Result<Async<usize>, Error>
    {
        assert!(end);
        assert!(data.len() == 0);
        Ok(Async::Ready(0))
    }
    fn start_response(&mut self, e: http::Encoder<S>) -> Reply<S> {
        let func = self.inner.take().expect("quick reply called only once");
        func(e)
    }
}
