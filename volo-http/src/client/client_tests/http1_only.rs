// FIXME:
//
// `httpbin.org` supports h2 (HTTP/2 with tls), but doesn't support h2c (HTTP/2 over Cleartext),
// just disable those test cases.
//
// TODO:
//
// Find a website that support h2c.

use std::{collections::HashMap, future::Future, time::Duration};

use bytes::Bytes;
use http::status::StatusCode;
use http_body_util::Full;
use motore::{
    layer::{Identity, Stack},
    service::Service,
};
use volo::context::Context;

use super::{
    HTTPBIN_GET, HttpBinResponse,
    utils::{
        AutoBody, AutoBodyLayer, AutoFull, AutoFullLayer, DropBodyLayer, Nothing,
        RespBodyToFullLayer,
    },
};
use crate::{
    ClientBuilder,
    body::{Body, BodyConversion},
    client::{
        CallOpt, Client,
        layer::{FailOnStatus, TargetLayer, http_proxy::HttpProxy},
        test_helpers::{DebugLayer, MockTransport, RetryOnStatus},
    },
    context::client::Config,
    error::ClientError,
    response::Response,
};

fn builder_for_debug()
-> ClientBuilder<Stack<Stack<RetryOnStatus, Identity>, DebugLayer>, Stack<HttpProxy, Identity>> {
    Client::builder()
        .layer_inner(RetryOnStatus::server_error())
        .layer_inner_front(DebugLayer::default())
        .layer_outer(HttpProxy::env())
}

#[tokio::test]
async fn client_with_generics() {
    // Override default `ReqBody`, but the `ReqBody` is still implements `http_body::Body`
    let _: Client<Full<Bytes>> = Client::builder().build().unwrap();

    // Override default `RespBody`, but the `RespBody` is still implements `http_body::Body`
    let _: Client<Body, Full<Bytes>> = Client::builder()
        .layer_outer_front(RespBodyToFullLayer)
        .build()
        .unwrap();

    // Override default `ReqBody` through `Layer`. The `AutoBody` does not implement
    // `http_body::Body`, but the `AutoBodyLayer` will convert it to `volo_http::body::Body` and
    // use it.
    let _: Client<AutoBody> = Client::builder()
        .layer_outer_front(AutoBodyLayer)
        .build()
        .unwrap();

    // Override default `ReqBody` through `Layer`. The `AutoFull` does not implement
    // `http_body::Body`, but the `AutoFullLayer` will convert it to `Full<Bytes>` which implements
    // `http_body::Body` as its `InnerReqBody`.
    let _: Client<AutoFull> = Client::builder()
        .layer_outer_front(AutoFullLayer)
        .build()
        .unwrap();

    // Override default `RespBody` through `Layer`. The `RespBody` does not implement
    // `http_body::Body`, but the `DropBodyLayer` will drop `volo_http::body::Body` and put
    // `Nothing` to `Response`.
    let _: Client<Body, Nothing> = Client::builder()
        .layer_outer_front(DropBodyLayer)
        .build()
        .unwrap();

    // Combine them
    let _: Client<AutoFull, Nothing> = Client::builder()
        .layer_outer_front(AutoFullLayer)
        .layer_outer_front(DropBodyLayer)
        .build()
        .unwrap();
}

#[cfg(feature = "json")]
#[tokio::test]
async fn client_test() {
    let client = builder_for_debug().build().unwrap();

    {
        let resp = client
            .get(HTTPBIN_GET)
            .send()
            .await
            .unwrap()
            .into_json::<HttpBinResponse>()
            .await
            .unwrap();
        assert!(resp.args.is_empty());
        assert_eq!(resp.url, HTTPBIN_GET);
    }
    {
        let resp = client
            .get("/get")
            .host("httpbin.org")
            .send()
            .await
            .unwrap()
            .into_json::<HttpBinResponse>()
            .await
            .unwrap();
        assert!(resp.args.is_empty());
        assert_eq!(resp.url, HTTPBIN_GET);
    }
}

#[cfg(feature = "json")]
#[tokio::test]
async fn client_target() {
    let client = builder_for_debug()
        .layer_outer_front(TargetLayer::new_host("httpbin.org"))
        .build()
        .unwrap();

    {
        let resp = client
            .get(HTTPBIN_GET)
            .send()
            .await
            .unwrap()
            .into_json::<HttpBinResponse>()
            .await
            .unwrap();
        assert!(resp.args.is_empty());
        assert_eq!(resp.url, HTTPBIN_GET);
    }
    {
        let resp = client
            .get("/get")
            .send()
            .await
            .unwrap()
            .into_json::<HttpBinResponse>()
            .await
            .unwrap();
        assert!(resp.args.is_empty());
        assert_eq!(resp.url, HTTPBIN_GET);
    }
}

fn test_data() -> HashMap<String, String> {
    HashMap::from([
        ("key1".to_string(), "val1".to_string()),
        ("key2".to_string(), "val2".to_string()),
    ])
}

#[cfg(all(feature = "query", feature = "json"))]
#[tokio::test]
async fn set_query() {
    let data = test_data();

    let client = builder_for_debug().build().unwrap();
    let resp = client
        .get("http://httpbin.org/get")
        .set_query(&data)
        .send()
        .await
        .unwrap()
        .into_json::<HttpBinResponse>()
        .await
        .unwrap();
    assert_eq!(resp.args, data);
}

#[cfg(all(feature = "form", feature = "json"))]
#[tokio::test]
async fn set_form() {
    let data = test_data();

    let client = builder_for_debug().build().unwrap();
    let resp = client
        .post("http://httpbin.org/post")
        .form(&data)
        .send()
        .await
        .unwrap()
        .into_json::<HttpBinResponse>()
        .await
        .unwrap();
    assert_eq!(resp.form, data);
}

#[cfg(feature = "json")]
#[tokio::test]
async fn set_json() {
    let data = test_data();

    let client = builder_for_debug().build().unwrap();
    let resp = client
        .post("http://httpbin.org/post")
        .json(&data)
        .send()
        .await
        .unwrap()
        .into_json::<HttpBinResponse>()
        .await
        .unwrap();
    assert_eq!(resp.json, Some(data));
}

struct GetTimeoutAsSeconds;

impl<Cx, Req> Service<Cx, Req> for GetTimeoutAsSeconds
where
    Cx: Context<Config = Config>,
{
    type Response = Response;
    type Error = ClientError;

    fn call(
        &self,
        cx: &mut Cx,
        _: Req,
    ) -> impl Future<Output = Result<Self::Response, Self::Error>> + Send {
        let timeout = cx.rpc_info().config().timeout();
        let resp = match timeout {
            Some(timeout) => {
                let secs = timeout.as_secs();
                Response::new(Body::from(format!("{secs}")))
            }
            None => {
                let mut resp = Response::new(Body::empty());
                *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                resp
            }
        };
        async { Ok(resp) }
    }
}

#[tokio::test]
async fn callopt_test() {
    let mut builder = builder_for_debug();
    builder.set_request_timeout(Duration::from_secs(1));
    let client = builder
        .layer_outer_front(FailOnStatus::server_error())
        .mock(MockTransport::service(GetTimeoutAsSeconds))
        .unwrap();
    // default timeout is 1 seconds
    assert_eq!(
        client
            .get("/")
            .send()
            .await
            .unwrap()
            .into_string()
            .await
            .unwrap(),
        "1"
    );
    // callopt set timeout to 5 seconds
    assert_eq!(
        client
            .get("/")
            .with_callopt(CallOpt::new().with_timeout(Duration::from_secs(5)))
            .send()
            .await
            .unwrap()
            .into_string()
            .await
            .unwrap(),
        "5"
    );
}

#[cfg(all(feature = "cookie", feature = "json"))]
#[tokio::test]
async fn cookie_store() {
    let client = builder_for_debug()
        .layer_inner(crate::client::cookie::CookieLayer::new(Default::default()))
        .layer_outer_front(crate::client::layer::TargetLayer::new_host("httpbin.org"))
        .build()
        .unwrap();

    // test server add cookie
    let resp = client
        .get("http://httpbin.org/cookies/set?key=value")
        .send()
        .await
        .unwrap();
    let cookies = resp
        .headers()
        .get_all(http::header::SET_COOKIE)
        .iter()
        .filter_map(|value| {
            std::str::from_utf8(value.as_bytes())
                .ok()
                .and_then(|val| cookie::Cookie::parse(val).map(|c| c.into_owned()).ok())
        })
        .collect::<Vec<_>>();
    assert_eq!(cookies[0].name(), "key");
    assert_eq!(cookies[0].value(), "value");

    #[derive(serde::Deserialize)]
    struct CookieResponse {
        #[serde(default)]
        cookies: HashMap<String, String>,
    }
    let resp = client
        .get("http://httpbin.org/cookies")
        .send()
        .await
        .unwrap();
    let json = resp.into_json::<CookieResponse>().await.unwrap();
    assert_eq!(json.cookies["key"], "value");

    // test server delete cookie
    _ = client
        .get("http://httpbin.org/cookies/delete?key")
        .send()
        .await
        .unwrap();
    let resp = client
        .get("http://httpbin.org/cookies")
        .send()
        .await
        .unwrap();
    let json = resp.into_json::<CookieResponse>().await.unwrap();
    assert_eq!(json.cookies.len(), 0);
}
