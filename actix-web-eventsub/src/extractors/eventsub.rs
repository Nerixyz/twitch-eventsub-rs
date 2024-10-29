//! This module contains the main `EventSub` extractor [`crate::Data`].

use crate::types::EventSubscription;
use actix_web::{dev, error::PayloadError, FromRequest, HttpRequest, ResponseError};
use bytes::BytesMut;
pub use eventsub_common::headers::{HeaderType, InvalidHeaders};
use eventsub_common::{
    headers,
    headers::{HeaderMapExt, PayloadHeaders},
    EventsubPayload, MessageType,
};
use futures_util::{future::Either, StreamExt};
use hmac::{
    digest::{generic_array::GenericArray, InvalidLength},
    Hmac, Mac,
};
use pin_project::pin_project;
use sha2::Sha256;
use std::{
    future::{ready, Future, Ready},
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

type HmacSha256 = Hmac<Sha256>;

/// Extractor for an eventsub event.
///
/// This will verify (hash, subscription-type, time, duplicate ids) and deserialize the event for you.
/// If you configured multiple events to arrive at the same endpoint,
/// use [`guards::event_type`](crate::guards::event_type) or [`guards::event_type_fn`](crate::guards::event_type_fn)
/// to guard each handler (see [multiple-actix](examples/multiple_actix.rs) example).
///
/// You need to provide a [`EventSubscription`] as the type of event you want to receive and a
/// [`Config`] that provides the secret and converts potential errors to your preferred error type.
///
/// Make sure that processing the event doesn't take too long, otherwise
/// twitch might revoke your subscription.
/// Consider doing expensive work in [`actix_web::rt::spawn`].
///
/// ```
/// # use actix_web::{HttpRequest, HttpResponse, Responder, web::{self, Data}};
/// # use actix_web_eventsub::{EventsubPayload, Verification, VerifyDecodeError, types::channel::ChannelPointsCustomRewardRedemptionAddV1};
/// # struct EventsubConfig;
/// #
/// # impl actix_web_eventsub::Config for EventsubConfig {
/// #     type Error = VerifyDecodeError;
/// #     type CheckEventIdFut = std::future::Ready<bool>;
/// #
/// #     fn get_secret(req: &HttpRequest) -> Result<&[u8], VerifyDecodeError> {
/// #         req.app_data::<Data<Vec<u8>>>()
/// #             .map(|v| v.as_slice())
/// #             .ok_or(VerifyDecodeError::NoHmacKey)
/// #     }
/// #
/// #     fn check_event_id(_req: &HttpRequest, _id: &str) -> Self::CheckEventIdFut {
/// #         std::future::ready(true)
/// #     }
/// #
/// #     fn convert_error(error: VerifyDecodeError) -> Self::Error {
/// #         error
/// #     }
/// # }
/// #
/// async fn event_handler(
///     event: actix_web_eventsub::Data<ChannelPointsCustomRewardRedemptionAddV1, EventsubConfig>,
/// ) -> impl Responder {
/// match event.payload {
///         EventsubPayload::Verification(Verification { challenge, .. }) => {
///             println!("Verification: {}", challenge);
///             HttpResponse::Ok()
///                 .content_type(mime::TEXT_PLAIN_UTF_8)
///                 .body(challenge)
///         }
///         x => {
///             println!("{:?}", x);
///             HttpResponse::NoContent().finish()
///         }
///     }
/// }
/// # fn main() {}
/// ```
pub struct Data<P, T> {
    /// The extracted payload.
    pub payload: EventsubPayload<P>,
    _config: PhantomData<T>,
}

/// Errors when verifying and decoding the eventsub payload.
#[derive(Debug, thiserror::Error, actix_web_error::Json)]
#[status(BAD_REQUEST)]
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
    PayloadError(PayloadError),
    /// serde_json couldn't deserialize the payload.
    #[error("JSON Deserialization error: {0}")]
    Serde(serde_json::Error),
    /// No HMAC key was provided - [`Config::get_secret`] returned [`None`].
    #[error("No HMAC key provided")]
    #[status(INTERNAL_SERVER_ERROR)]
    NoHmacKey,
    /// The HMAC key was too short - [`Config::get_secret`] returned a slice that was too short.
    #[error("Bad secret key")]
    #[status(INTERNAL_SERVER_ERROR)]
    HmacInit(InvalidLength),
    /// The subscription version didn't match the expected one.
    #[error("Version mismatch - expected {0}")]
    VersionMismatch(&'static str),
    /// The message id wasn't valid utf8
    #[error("The message id wasn't valid utf8")]
    IdNotUtf8,
    /// This message won't be handled because [`Config::check_event_id`] resolved to `false`.
    #[error("Won't handle id (possible duplicate)")]
    WontHandleId,
}

/// Configuration for verifying and decoding eventsub payloads.
pub trait Config {
    /// Preferred error type (see [`Config::convert_error`]).
    ///
    /// If you don't care about the error, set this to [`VerifyDecodeError`].
    type Error: ResponseError;

    /// [`Future`] returned from [`Self::check_event_id`]
    type CheckEventIdFut: Future<Output = bool> + 'static;

    /// Get the eventsub secret.
    ///
    /// This should always return [`Ok`].
    ///
    /// ## Errors
    ///
    /// If you can't get the secret, return an error instead of panicking.
    fn get_secret(req: &HttpRequest) -> Result<&[u8], Self::Error>;

    /// Check if you've already seen this id.
    ///
    /// The returned [`Future`] should resolve to `true` if you want to handle this event
    /// (i.e. you haven't seen the id in the last ≈10min).
    fn check_event_id(req: &HttpRequest, id: &str) -> Self::CheckEventIdFut;

    /// Convert the [`VerifyDecodeError`] into a custom error.
    ///
    /// If you want to return a custom error (for example an error wrapped in JSON),
    /// then you should construct it here. Otherwise, return the given error.
    fn convert_error(error: VerifyDecodeError) -> Self::Error;
}

impl<P, T> FromRequest for Data<P, T>
where
    T: Config,
    P: EventSubscription,
    T::Error: 'static,
{
    type Error = T::Error;
    type Future = Either<Ready<Result<Self, Self::Error>>, VerifyDecodeFut<P, T>>;

    fn from_request(req: &HttpRequest, payload: &mut dev::Payload) -> Self::Future {
        let parsed = match headers::read_eventsub_headers::<_, P>(req.headers())
            .map_err(VerifyDecodeError::Headers)
            .map_err(T::convert_error)
        {
            Ok(h) => h,
            Err(e) => return Either::Left(ready(Err(e))),
        };
        match init_mac::<T>(req, parsed.id_bytes, parsed.timestamp_bytes) {
            Ok(mac) => Either::Right(VerifyDecodeFut::DecodingResponse {
                payload: dev::Payload::take(payload),
                mac,
                bytes: BytesMut::new(),
                headers: parsed.payload,
                req: req.clone(),
            }),
            Err(e) => Either::Left(ready(Err(e))),
        }
    }
}

fn init_mac<T: Config>(
    req: &HttpRequest,
    id_bytes: &[u8],
    timestamp_bytes: &[u8],
) -> Result<HmacSha256, T::Error> {
    let mut mac = HmacSha256::new_from_slice(T::get_secret(req)?)
        .map_err(VerifyDecodeError::HmacInit)
        .map_err(T::convert_error)?;
    mac.update(id_bytes);
    mac.update(timestamp_bytes);

    Ok(mac)
}

/// A future for verifying an EventSub payload.
#[pin_project(project = VerifyDecodeProj)]
pub enum VerifyDecodeFut<P, T: Config> {
    /// Step 1: decoding/reading the response
    DecodingResponse {
        /// Payload(-stream)
        payload: dev::Payload,
        /// Hmac state
        mac: HmacSha256,
        /// Decoded data
        bytes: BytesMut,
        /// Initial header information
        headers: PayloadHeaders,
        /// Reference to HttpRequest (an Rc internally, but we drop it after decoding)
        req: HttpRequest,
    },
    /// Step 2: checking the id of this payload
    CheckingId {
        /// The decoded payload, always [`Some`] until this future completes.
        payload: Option<Data<P, T>>,
        /// Future of checking the event id
        #[pin]
        inner: T::CheckEventIdFut,
    },
}

const EMPTY_KEY: [u8; 64] = [0u8; 64];

impl<P, T> Future for VerifyDecodeFut<P, T>
where
    P: EventSubscription,
    T: Config,
{
    type Output = Result<Data<P, T>, T::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        'outer: loop {
            match self.as_mut().project() {
                VerifyDecodeProj::DecodingResponse {
                    payload,
                    bytes,
                    mac,
                    headers,
                    req,
                } => loop {
                    match Pin::new(&mut payload.next()).poll(cx) {
                        Poll::Ready(Some(Ok(ref chunk))) => {
                            if bytes.len() >= 10_000_000 {
                                break 'outer Poll::Ready(Err(T::convert_error(
                                    VerifyDecodeError::RequestTooLarge,
                                )));
                            }
                            bytes.extend_from_slice(chunk);
                            mac.update(chunk);
                        }
                        Poll::Ready(Some(Err(e))) => {
                            break 'outer Poll::Ready(Err(T::convert_error(
                                VerifyDecodeError::PayloadError(e),
                            )))
                        }
                        Poll::Ready(None) => {
                            let signature = std::mem::replace(
                                mac,
                                HmacSha256::new(GenericArray::from_slice(&EMPTY_KEY)),
                            );

                            if signature.verify_slice(&headers.signature).is_err() {
                                break 'outer Poll::Ready(Err(T::convert_error(
                                    VerifyDecodeError::SignatureMismatch,
                                )));
                            }
                            let payload_result =
                                match headers.message_type {
                                    MessageType::Verification => serde_json::from_slice(bytes)
                                        .map(EventsubPayload::Verification),
                                    MessageType::Revocation => serde_json::from_slice(bytes)
                                        .map(EventsubPayload::Revocation),
                                    MessageType::Notification => serde_json::from_slice(bytes)
                                        .map(EventsubPayload::Notification),
                                }
                                .map(|payload| Data {
                                    payload,
                                    _config: PhantomData,
                                })
                                .map_err(VerifyDecodeError::Serde);
                            let id_header = req
                                .headers()
                                .get_message_id()
                                .unwrap()
                                .to_str()
                                .map_err(|_| VerifyDecodeError::IdNotUtf8);
                            match (payload_result, id_header) {
                                (Ok(payload), Ok(id)) => {
                                    let inner = T::check_event_id(req, id);
                                    self.set(VerifyDecodeFut::CheckingId {
                                        payload: Some(payload),
                                        inner,
                                    });
                                    continue 'outer;
                                }
                                (Err(e), _) | (Ok(_), Err(e)) => {
                                    break 'outer Poll::Ready(Err(T::convert_error(e)))
                                }
                            }
                        }
                        Poll::Pending => break 'outer Poll::Pending,
                    }
                },
                VerifyDecodeProj::CheckingId { inner, payload } => {
                    break 'outer match inner.poll(cx) {
                        Poll::Ready(true) => Poll::Ready(Ok(payload.take().unwrap())),
                        Poll::Ready(false) => {
                            Poll::Ready(Err(T::convert_error(VerifyDecodeError::WontHandleId)))
                        }
                        Poll::Pending => Poll::Pending,
                    }
                }
            }
        }
    }
}
