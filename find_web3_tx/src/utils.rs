use std::time::Duration;

use ethers::prelude::{HttpClientError, JsonRpcError, RetryPolicy};
use serde::Deserialize;

#[derive(Debug, Default)]
pub struct CustomRetryPolicy;

impl RetryPolicy<HttpClientError> for CustomRetryPolicy {
    fn should_retry(&self, error: &HttpClientError) -> bool {
        fn should_retry_json_rpc_error(err: &JsonRpcError) -> bool {
            let JsonRpcError { code, message, .. } = err;
            // alchemy throws it this way
            if *code == 429 {
                return true;
            }

            // This is an infura error code for `exceeded project rate limit`
            if *code == -32005 {
                return true;
            }

            // alternative alchemy error for specific IPs
            if *code == -32016 && message.contains("rate limit") {
                return true;
            }

            match message.as_str() {
                // this is commonly thrown by infura and is apparently a load balancer issue, see also <https://github.com/MetaMask/metamask-extension/issues/7234>
                "header not found" => true,
                // also thrown by infura if out of budget for the day and ratelimited
                "daily request count exceeded, request rate limited" => true,
                _ => false,
            }
        }

        match error {
            HttpClientError::ReqwestError(_) => true,
            HttpClientError::JsonRpcError(err) => should_retry_json_rpc_error(err),
            HttpClientError::SerdeJson { text, .. } => {
                // some providers send invalid JSON RPC in the error case (no `id:u64`), but the
                // text should be a `JsonRpcError`
                #[derive(Deserialize)]
                struct Resp {
                    error: JsonRpcError,
                }

                if let Ok(resp) = serde_json::from_str::<Resp>(text) {
                    return should_retry_json_rpc_error(&resp.error);
                }
                false
            }
        }
    }

    fn backoff_hint(&self, error: &HttpClientError) -> Option<Duration> {
        if let HttpClientError::JsonRpcError(JsonRpcError { data, .. }) = error {
            let data = data.as_ref()?;

            // if daily rate limit exceeded, infura returns the requested backoff in the error
            // response
            let backoff_seconds = &data["rate"]["backoff_seconds"];
            // infura rate limit error
            if let Some(seconds) = backoff_seconds.as_u64() {
                return Some(Duration::from_secs(seconds));
            }
            if let Some(seconds) = backoff_seconds.as_f64() {
                return Some(Duration::from_secs(seconds as u64 + 1));
            }
        }

        None
    }
}
