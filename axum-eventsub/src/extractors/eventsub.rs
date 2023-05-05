use axum::{
    body::HttpBody,
    extract::{rejection::BytesRejection, FromRequest},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    BoxError,
};
use bytes::Bytes;
pub use eventsub_common::headers::{HeaderType, InvalidHeaders};
use eventsub_common::{headers, types::EventSubscription, EventsubPayload, MessageType};
use hmac::{digest::InvalidLength, Hmac, Mac};
use sha2::Sha256;
use std::marker::PhantomData;

type HmacSha256 = Hmac<Sha256>;

pub struct Data<P, C> {
    /// The extracted payload.
    pub payload: EventsubPayload<P>,
    _config: PhantomData<C>,
}

/// Configuration for verifying and decoding eventsub payloads.
///
/// The config is generic over the app state (`S`).
pub trait Config<S> {
    /// Preferred rejection (see [`Config::convert_error`]).
    ///
    /// If you don't care about the error, set this to [`VerifyDecodeError`].
    type Rejection: IntoResponse;

    /// Get the eventsub secret from the app state.
    fn get_secret(state: &S) -> &[u8];

    /// Convert the [`VerifyDecodeError`] into a custom error.
    ///
    /// If you want to return a custom rejection (for example an error wrapped in JSON),
    /// then you should construct it here. Otherwise, return the given error.
    fn convert_error(error: VerifyDecodeError) -> Self::Rejection;
}

/// Errors when verifying and decoding the eventsub payload.
#[derive(Debug, thiserror::Error)]
pub enum VerifyDecodeError {
    /// An issue with the headers. See [`eventsub_common::headers::InvalidHeaders`] for more detail.
    #[error("Invalid headers: {0}")]
    Headers(InvalidHeaders),
    /// The provided signature was incorrect - it didn't match the computed one.
    #[error("The provided signature wasn't expected")]
    SignatureMismatch,
    /// The payload was too large (>10MB).
    #[error("The request was too large (> 10MB)")]
    RequestTooLarge,
    /// actix-web couldn't parse the payload.
    #[error("Payload error: {0}")]
    PayloadError(BytesRejection),
    /// serde_json couldn't deserialize the payload.
    #[error("JSON Deserialization error: {0}")]
    Serde(serde_json::Error),
    /// The HMAC key was too short - [`Config::get_secret`] returned a slice that was too short.
    #[error("Bad secret key")]
    HmacInit(InvalidLength),
    /// The subscription version didn't match the expected one.
    #[error("Version mismatch - expected {0}")]
    VersionMismatch(&'static str),
}

#[async_trait::async_trait]
impl<State, Sub, C, B> FromRequest<State, B> for Data<Sub, C>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
    C: Config<State>,
    Sub: EventSubscription,
    State: std::marker::Send + std::marker::Sync,
{
    type Rejection = C::Rejection;

    async fn from_request(req: Request<B>, state: &State) -> Result<Self, Self::Rejection> {
        let headers = headers::read_eventsub_headers::<_, Sub>(req.headers())
            .map_err(|e| C::convert_error(VerifyDecodeError::Headers(e)))?;
        let mut mac = init_mac::<State, C>(state, headers.id_bytes, headers.timestamp_bytes)
            .map_err(C::convert_error)?;
        let payload_headers = headers.payload;
        let payload = Bytes::from_request(req, state)
            .await
            .map_err(|e| C::convert_error(VerifyDecodeError::PayloadError(e)))?;
        mac.update(&payload);
        let computed = mac.finalize().into_bytes();

        if AsRef::<[u8; 32]>::as_ref(&computed) == payload_headers.signature.as_slice() {
            match payload_headers.message_type {
                MessageType::Verification => {
                    serde_json::from_slice(&payload).map(EventsubPayload::Verification)
                }
                MessageType::Revocation => {
                    serde_json::from_slice(&payload).map(EventsubPayload::Revocation)
                }
                MessageType::Notification => {
                    serde_json::from_slice(&payload).map(EventsubPayload::Notification)
                }
            }
            .map(|payload| Data {
                payload,
                _config: PhantomData,
            })
            .map_err(|e| C::convert_error(VerifyDecodeError::Serde(e)))
        } else {
            Err(C::convert_error(VerifyDecodeError::SignatureMismatch))
        }
    }
}

fn init_mac<S, T: Config<S>>(
    state: &S,
    id_bytes: &[u8],
    timestamp_bytes: &[u8],
) -> Result<HmacSha256, VerifyDecodeError> {
    let mut mac =
        HmacSha256::new_from_slice(T::get_secret(state)).map_err(VerifyDecodeError::HmacInit)?;
    mac.update(id_bytes);
    mac.update(timestamp_bytes);

    Ok(mac)
}

impl IntoResponse for VerifyDecodeError {
    fn into_response(self) -> Response {
        let status = match &self {
            VerifyDecodeError::Headers(_)
            | VerifyDecodeError::SignatureMismatch
            | VerifyDecodeError::RequestTooLarge
            | VerifyDecodeError::PayloadError(_)
            | VerifyDecodeError::Serde(_)
            | VerifyDecodeError::VersionMismatch(_) => StatusCode::BAD_REQUEST,
            VerifyDecodeError::HmacInit(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, self.to_string()).into_response()
    }
}
