// Copyright (c) Microsoft. All rights reserved.

pub(super) fn handle(
    req: hyper::Request<hyper::Body>,
    inner: std::sync::Arc<futures_util::lock::Mutex<aziot_identityd::Server>>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<hyper::Response<hyper::Body>, hyper::Request<hyper::Body>>> + Send>> {
    Box::pin(async move {
        if req.uri().path() != "/identities/modules" {
            return Err(req);
        }
        
        let mut inner = inner.lock().await;
        let inner = &mut *inner;

        let user = aziot_identityd::auth::Uid(0);
        let auth_id = match inner.authenticator.authenticate(user) {
            Ok(auth_id) => auth_id,
            Err(err) => return Ok(super::ToHttpResponse::to_http_response(&err)),
        };

        let (http::request::Parts { method, headers, .. }, body) = req.into_parts();
        let content_type = headers.get(hyper::header::CONTENT_TYPE).and_then(|value| value.to_str().ok());

        match method {
            hyper::Method::GET => {
                //TODO: get uid from UDS
                let response = match inner.get_identities(auth_id, "aziot").await {
                    Ok(v) => v,
                    Err(err) => return Ok(super::ToHttpResponse::to_http_response(&err)),
                };
                let response = aziot_identity_common_http::get_module_identities::Response { identities: response };
        
                let response = super::json_response(hyper::StatusCode::OK, &response);
                Ok(response)
            },
            hyper::Method::POST => {
                if content_type.as_deref() != Some("application/json") {
                    return Ok(super::err_response(
                        hyper::StatusCode::UNSUPPORTED_MEDIA_TYPE,
                        None,
                        "request body must be application/json".into(),
                    ));
                }
        
                let body = match hyper::body::to_bytes(body).await {
                    Ok(body) => body,
                    Err(err) => return Ok(super::err_response(
                        hyper::StatusCode::BAD_REQUEST,
                        None,
                        super::error_to_message(&err).into(),
                    )),
                };
        
                let body: aziot_identity_common_http::create_module_identity::Request = match serde_json::from_slice(&body) {
                    Ok(body) => body,
                    Err(err) => return Ok(super::err_response(
                        hyper::StatusCode::UNPROCESSABLE_ENTITY,
                        None,
                        super::error_to_message(&err).into(),
                    )),
                };
        
                //TODO: get uid from UDS
                let id = match inner.create_identity(auth_id, &body.id_type, &body.module_id).await {
                    Ok(id) => id,
                    Err(err) => return Ok(super::ToHttpResponse::to_http_response(&err)),
                };
        
                let response = aziot_identity_common_http::create_module_identity::Response {
                    identity: id
                };
        
                let response = super::json_response(hyper::StatusCode::OK, &response);
                Ok(response)
            },
            _ => {
                return Ok(super::err_response(
                    hyper::StatusCode::METHOD_NOT_ALLOWED,
                    Some((hyper::header::ALLOW, "GET, POST")),
                    "method not allowed".into(),
                ));
            }
        }
    })
}