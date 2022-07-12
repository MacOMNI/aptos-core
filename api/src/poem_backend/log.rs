// Copyright (c) Aptos
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use crate::metrics::RESPONSE_STATUS;
use aptos_logger::{
    debug, error,
    prelude::{sample, SampleRate},
    sample::Sampling,
    Schema,
};
use poem::{
    http::{header, StatusCode},
    Endpoint, IntoResponse, Request, Response, Result,
};

/// Logs information about the request and response if the response status code
/// is >= 500, to help us debug since this will be an error on our side.
/// We also do general logging of the status code alone regardless of what it is.
pub async fn middleware_log<E: Endpoint>(next: E, request: Request) -> Result<Response> {
    let start = std::time::Instant::now();

    let mut log = HttpRequestLog {
        remote_addr: request.remote_addr().as_socket_addr().cloned(),
        method: request.method().to_string(),
        path: request.uri().path().to_string(),
        status: 0,
        referer: request
            .headers()
            .get(header::REFERER)
            .and_then(|v| v.to_str().ok().map(|v| v.to_string())),
        user_agent: request
            .headers()
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok().map(|v| v.to_string())),
        elapsed: Duration::from_secs(0),
        forwarded: request
            .headers()
            .get(header::FORWARDED)
            .and_then(|v| v.to_str().ok().map(|v| v.to_string())),
    };

    let result = next.call(request).await;

    let (out, status_code) = match result {
        Ok(response) => {
            let response = response.into_response();
            let status_code = response.status().as_u16();
            (Ok(response), status_code)
        }
        // TODO: Figure out how to get the status code from the error without
        // destroying it, since we need it for the response.
        Err(err) => (Err(err), StatusCode::INTERNAL_SERVER_ERROR.as_u16()),
    };

    let elapsed = start.elapsed();

    log.status = status_code;
    log.elapsed = elapsed;

    if status_code >= 500 {
        sample!(SampleRate::Duration(Duration::from_secs(1)), error!(log));
    } else {
        debug!(log);
    }

    RESPONSE_STATUS
        .with_label_values(&[status_code.to_string().as_str()])
        .observe(elapsed.as_secs_f64());

    out
}

// TODO: Figure out how to have certain fields be borrowed, like in the
// original implementation.
#[derive(Schema)]
pub struct HttpRequestLog {
    #[schema(display)]
    remote_addr: Option<std::net::SocketAddr>,
    method: String,
    path: String,
    pub status: u16,
    referer: Option<String>,
    user_agent: Option<String>,
    #[schema(debug)]
    pub elapsed: std::time::Duration,
    forwarded: Option<String>,
}
