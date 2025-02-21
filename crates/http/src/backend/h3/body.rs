//! Types for parsing HTTP/3 bodies.
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Buf, Bytes};
use h3::server::RequestStream;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Data(Option<u64>),
    Trailers,
    Done,
}

/// An incoming HTTP/3 body.
///
/// Implements [`http_body::Body`].
pub struct QuicIncomingBody<S> {
    stream: RequestStream<S, Bytes>,
    state: State,
}

impl<S> QuicIncomingBody<S> {
    /// Create a new incoming HTTP/3 body.
    pub fn new(stream: RequestStream<S, Bytes>, size_hint: Option<u64>) -> Self {
        Self {
            stream,
            state: State::Data(size_hint),
        }
    }
}

impl<S: h3::quic::RecvStream> http_body::Body for QuicIncomingBody<S> {
    type Data = Bytes;
    type Error = h3::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let QuicIncomingBody { stream, state } = self.as_mut().get_mut();

        if *state == State::Done {
            return Poll::Ready(None);
        }

        if let State::Data(remaining) = state {
            match stream.poll_recv_data(cx) {
                Poll::Ready(Ok(Some(mut buf))) => {
                    let buf_size = buf.remaining() as u64;

                    if let Some(remaining) = remaining {
                        if buf_size > *remaining {
                            *state = State::Done;
                            return Poll::Ready(Some(Err(h3::error::Code::H3_FRAME_UNEXPECTED.into())));
                        }

                        *remaining -= buf_size;
                    }

                    return Poll::Ready(Some(Ok(http_body::Frame::data(buf.copy_to_bytes(buf_size as usize)))));
                }
                Poll::Ready(Ok(None)) => {
                    *state = State::Trailers;
                }
                Poll::Ready(Err(err)) => {
                    *state = State::Done;
                    return Poll::Ready(Some(Err(err)));
                }
                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }

        // We poll the recv data again even though we already got the None
        // because we want to make sure there is not a frame after the trailers
        // This is a workaround because h3 does not allow us to poll the trailer
        // directly, so we need to make sure the future recv_trailers is going to be
        // ready after a single poll We avoid pinning to the heap.
        let resp = match stream.poll_recv_data(cx) {
            Poll::Ready(Ok(None)) => match std::pin::pin!(stream.recv_trailers()).poll(cx) {
                Poll::Ready(Ok(Some(trailers))) => Poll::Ready(Some(Ok(http_body::Frame::trailers(trailers)))),
                // We will only poll the recv_trailers once so if pending is returned we are done.
                Poll::Pending => {
                    #[cfg(feature = "tracing")]
                    tracing::warn!("recv_trailers is pending");
                    Poll::Ready(None)
                }
                Poll::Ready(Ok(None)) => Poll::Ready(None),
                Poll::Ready(Err(err)) => Poll::Ready(Some(Err(err))),
            },
            // We are not expecting any data after the previous poll returned None
            Poll::Ready(Ok(Some(_))) => Poll::Ready(Some(Err(h3::error::Code::H3_FRAME_UNEXPECTED.into()))),
            Poll::Ready(Err(err)) => Poll::Ready(Some(Err(err))),
            Poll::Pending => return Poll::Pending,
        };

        *state = State::Done;

        resp
    }

    fn size_hint(&self) -> http_body::SizeHint {
        match self.state {
            State::Data(Some(remaining)) => http_body::SizeHint::with_exact(remaining),
            State::Data(None) => http_body::SizeHint::default(),
            State::Trailers | State::Done => http_body::SizeHint::with_exact(0),
        }
    }

    fn is_end_stream(&self) -> bool {
        match self.state {
            State::Data(Some(0)) | State::Trailers | State::Done => true,
            State::Data(_) => false,
        }
    }
}
