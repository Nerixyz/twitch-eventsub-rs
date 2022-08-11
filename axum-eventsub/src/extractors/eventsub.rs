use axum::{
    body::HttpBody,
    extract::{rejection::BytesRejection, FromRequest, RequestParts},
    http::{Extensions, StatusCode},
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
pub trait Config {
    /// Preferred rejection (see [`Config::convert_error`]).
    ///
    /// If you don't care about the error, set this to [`VerifyDecodeError`].
    type Rejection: IntoResponse;

    /// Get the eventsub secret.
    ///
    /// This should always return [`Some`], but if you can't get
    /// the secret at all, return [`None`] to avoid panicking.
    fn get_secret(ext: &Extensions) -> Option<&[u8]>;

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
    /// No HMAC key was provided - [`Config::get_secret`] returned [`None`].
    #[error("No HMAC key provided")]
    NoHmacKey,
    /// The HMAC key was too short - [`Config::get_secret`] returned a slice that was too short.
    #[error("Bad secret key")]
    HmacInit(InvalidLength),
    /// The subscription version didn't match the expected one.
    #[error("Version mismatch - expected {0}")]
    VersionMismatch(&'static str),
}

#[async_trait::async_trait]
impl<S, C, B> FromRequest<B> for Data<S, C>
where
    B: HttpBody + Send,
    B::Data: Send,
    B::Error: Into<BoxError>,
    C: Config,
    S: EventSubscription,
{
    type Rejection = C::Rejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let headers = headers::read_eventsub_headers::<_, S>(req.headers())
            .map_err(|e| C::convert_error(VerifyDecodeError::Headers(e)))?;
        let mut mac = init_mac::<C>(req.extensions(), headers.id_bytes, headers.timestamp_bytes)
            .map_err(C::convert_error)?;
        let payload_headers = headers.payload;
        let payload = Bytes::from_request(req)
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

fn init_mac<T: Config>(
    ext: &Extensions,
    id_bytes: &[u8],
    timestamp_bytes: &[u8],
) -> Result<HmacSha256, VerifyDecodeError> {
    let mut mac =
        HmacSha256::new_from_slice(T::get_secret(ext).ok_or(VerifyDecodeError::NoHmacKey)?)
            .map_err(VerifyDecodeError::HmacInit)?;
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
            VerifyDecodeError::NoHmacKey | VerifyDecodeError::HmacInit(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };

        (status, self.to_string()).into_response()
    }
}
