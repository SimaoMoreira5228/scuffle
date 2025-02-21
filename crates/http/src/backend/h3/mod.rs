//! HTTP3 backend.
use std::fmt::Debug;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use body::QuicIncomingBody;
use scuffle_context::ContextFutExt;
#[cfg(feature = "tracing")]
use tracing::Instrument;
use utils::copy_response_body;

use crate::error::Error;
use crate::service::{HttpService, HttpServiceFactory};

pub mod body;
mod utils;

/// A backend that handles incoming HTTP3 connections.
///
/// This is used internally by the [`HttpServer`](crate::server::HttpServer) but can be used directly if preferred.
///
/// Call [`run`](Http3Backend::run) to start the server.
#[derive(bon::Builder, Debug, Clone)]
pub struct Http3Backend<F> {
    /// The [`scuffle_context::Context`] this server will live by.
    #[builder(default = scuffle_context::Context::global())]
    ctx: scuffle_context::Context,
    /// The number of worker tasks to spawn for each server backend.
    #[builder(default = 1)]
    worker_tasks: usize,
    /// The service factory that will be used to create new services.
    service_factory: F,
    /// The address to bind to.
    ///
    /// Use `[::]` for a dual-stack listener.
    /// For example, use `[::]:80` to bind to port 80 on both IPv4 and IPv6.
    bind: SocketAddr,
    /// rustls config.
    ///
    /// Use this field to set the server into TLS mode.
    /// It will only accept TLS connections when this is set.
    rustls_config: rustls::ServerConfig,
}

impl<F> Http3Backend<F>
where
    F: HttpServiceFactory + Clone + Send + 'static,
    F::Error: std::error::Error + Send,
    F::Service: Clone + Send + 'static,
    <F::Service as HttpService>::Error: std::error::Error + Send + Sync,
    <F::Service as HttpService>::ResBody: Send,
    <<F::Service as HttpService>::ResBody as http_body::Body>::Data: Send,
    <<F::Service as HttpService>::ResBody as http_body::Body>::Error: std::error::Error + Send + Sync,
{
    /// Run the HTTP3 server
    ///
    /// This function will bind to the address specified in `bind`, listen for incoming connections and handle requests.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all, fields(bind = %self.bind)))]
    pub async fn run(mut self) -> Result<(), Error<F>> {
        #[cfg(feature = "tracing")]
        tracing::debug!("starting server");

        // not quite sure why this is necessary but it is
        self.rustls_config.max_early_data_size = u32::MAX;
        let crypto = h3_quinn::quinn::crypto::rustls::QuicServerConfig::try_from(self.rustls_config)?;
        let server_config = h3_quinn::quinn::ServerConfig::with_crypto(Arc::new(crypto));

        // Bind the UDP socket
        let socket = std::net::UdpSocket::bind(self.bind)?;

        // Runtime for the quinn endpoint
        let runtime = h3_quinn::quinn::default_runtime()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no async runtime found"))?;

        // Create a child context for the workers so we can shut them down if one of them fails without shutting down the main context
        let (worker_ctx, worker_handler) = self.ctx.new_child();

        let workers = (0..self.worker_tasks).map(|_n| {
            let ctx = worker_ctx.clone();
            let service_factory = self.service_factory.clone();
            let server_config = server_config.clone();
            let socket = socket.try_clone().expect("failed to clone socket");
            let runtime = Arc::clone(&runtime);

            let worker_fut = async move {
                let endpoint = h3_quinn::quinn::Endpoint::new(
                    h3_quinn::quinn::EndpointConfig::default(),
                    Some(server_config),
                    socket,
                    runtime,
                )?;

                #[cfg(feature = "tracing")]
                tracing::trace!("waiting for connections");

                while let Some(Some(new_conn)) = endpoint.accept().with_context(&ctx).await {
                    let mut service_factory = service_factory.clone();
                    let ctx = ctx.clone();

                    tokio::spawn(async move {
                        let _res: Result<_, Error<F>> = async move {
                            let Some(conn) = new_conn.with_context(&ctx).await.transpose()? else {
                                #[cfg(feature = "tracing")]
                                tracing::trace!("context done while accepting connection");
                                return Ok(());
                            };
                            let addr = conn.remote_address();

                            #[cfg(feature = "tracing")]
                            tracing::debug!(addr = %addr, "accepted quic connection");

                            let connection_fut = async move {
                                let Some(mut h3_conn) = h3::server::Connection::new(h3_quinn::Connection::new(conn))
                                    .with_context(&ctx)
                                    .await
                                    .transpose()?
                                else {
                                    #[cfg(feature = "tracing")]
                                    tracing::trace!("context done while establishing connection");
                                    return Ok(());
                                };

                                // make a new service for this connection
                                let http_service = service_factory
                                    .new_service(addr)
                                    .await
                                    .map_err(|e| Error::ServiceFactoryError(e))?;

                                loop {
                                    match h3_conn.accept().with_context(&ctx).await {
                                        Some(Ok(Some((req, stream)))) => {
                                            #[cfg(feature = "tracing")]
                                            tracing::debug!(method = %req.method(), uri = %req.uri(), "received request");

                                            let (mut send, recv) = stream.split();

                                            let size_hint = req
                                                .headers()
                                                .get(http::header::CONTENT_LENGTH)
                                                .and_then(|len| len.to_str().ok().and_then(|x| x.parse().ok()));
                                            let body = QuicIncomingBody::new(recv, size_hint);
                                            let req = req.map(|_| crate::body::IncomingBody::from(body));

                                            let ctx = ctx.clone();
                                            let mut http_service = http_service.clone();
                                            tokio::spawn(async move {
                                                let _res: Result<_, Error<F>> = async move {
                                                    let resp =
                                                        http_service.call(req).await.map_err(|e| Error::ServiceError(e))?;
                                                    let (parts, body) = resp.into_parts();

                                                    send.send_response(http::Response::from_parts(parts, ())).await?;
                                                    copy_response_body(send, body).await?;

                                                    Ok(())
                                                }
                                                .await;

                                                #[cfg(feature = "tracing")]
                                                if let Err(e) = _res {
                                                    tracing::warn!(err = %e, "error handling request");
                                                }

                                                // This moves the context into the async block because it is dropped here
                                                drop(ctx);
                                            });
                                        }
                                        // indicating no more streams to be received
                                        Some(Ok(None)) => {
                                            break;
                                        }
                                        Some(Err(err)) => match err.get_error_level() {
                                            h3::error::ErrorLevel::ConnectionError => return Err(err.into()),
                                            h3::error::ErrorLevel::StreamError => {
                                                #[cfg(feature = "tracing")]
                                                tracing::warn!("error on accept: {}", err);
                                                continue;
                                            }
                                        },
                                        // context is done
                                        None => {
                                            #[cfg(feature = "tracing")]
                                            tracing::trace!("context done, stopping connection loop");
                                            break;
                                        }
                                    }
                                }

                                #[cfg(feature = "tracing")]
                                tracing::trace!("connection closed");

                                Ok(())
                            };

                            #[cfg(feature = "tracing")]
                            let connection_fut = connection_fut.instrument(tracing::trace_span!("connection", addr = %addr));

                            connection_fut.await
                        }
                        .await;

                        #[cfg(feature = "tracing")]
                        if let Err(err) = _res {
                            tracing::warn!(err = %err, "error handling connection");
                        }
                    });
                }

                // shut down gracefully
                // wait for connections to be closed before exiting
                endpoint.wait_idle().await;

                Ok::<_, crate::error::Error<F>>(())
            };

            #[cfg(feature = "tracing")]
            let worker_fut = worker_fut.instrument(tracing::trace_span!("worker", n = _n));

            tokio::spawn(worker_fut)
        });

        if let Err(_e) = futures::future::try_join_all(workers).await {
            #[cfg(feature = "tracing")]
            tracing::error!(err = %_e, "error running workers");
        }

        drop(worker_ctx);
        worker_handler.shutdown().await;

        #[cfg(feature = "tracing")]
        tracing::debug!("all workers finished");

        Ok(())
    }
}
