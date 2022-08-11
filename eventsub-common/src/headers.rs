use crate::{types::EventSubscription, MessageType};
use chrono::{DateTime, Duration, Utc};
use http::HeaderValue;
use std::str::FromStr;

pub const SUBSCRIPTION_TYPE: &str = "Twitch-Eventsub-Subscription-Type";
pub const SUBSCRIPTION_VERSION: &str = "Twitch-Eventsub-Subscription-Version";
pub const MESSAGE_SIGNATURE: &str = "Twitch-Eventsub-Message-Signature";
pub const MESSAGE_TYPE: &str = "Twitch-Eventsub-Message-Type";
pub const MESSAGE_ID: &str = "Twitch-Eventsub-Message-Id";
pub const MESSAGE_TIMESTAMP: &str = "Twitch-Eventsub-Message-Timestamp";

pub trait HeaderMapExt {
    fn get(&self, key: &str) -> Option<&HeaderValue>;

    fn get_subscription_type(&self) -> Result<&HeaderValue, InvalidHeaders> {
        self.get(SUBSCRIPTION_TYPE)
            .ok_or(InvalidHeaders::Missing(HeaderType::SubscriptionType))
    }
    fn get_subscription_version(&self) -> Result<&HeaderValue, InvalidHeaders> {
        self.get(SUBSCRIPTION_VERSION)
            .ok_or(InvalidHeaders::Missing(HeaderType::SubscriptionVersion))
    }
    fn get_signature(&self) -> Result<&HeaderValue, InvalidHeaders> {
        self.get(MESSAGE_SIGNATURE)
            .ok_or(InvalidHeaders::Missing(HeaderType::Signature))
    }
    fn get_message_type(&self) -> Result<MessageType, InvalidHeaders> {
        self.get(MESSAGE_TYPE)
            .ok_or(InvalidHeaders::Missing(HeaderType::MessageType))?
            .try_into()
            .map_err(|_| InvalidHeaders::BadMessageType)
    }
    fn get_message_id(&self) -> Result<&HeaderValue, InvalidHeaders> {
        self.get(MESSAGE_ID)
            .ok_or(InvalidHeaders::Missing(HeaderType::Id))
    }
    fn get_message_timestamp(&self) -> Result<&HeaderValue, InvalidHeaders> {
        self.get(MESSAGE_TIMESTAMP)
            .ok_or(InvalidHeaders::Missing(HeaderType::Timestamp))
    }
}

impl HeaderMapExt for http::HeaderMap {
    fn get(&self, key: &str) -> Option<&HeaderValue> {
        self.get(key)
    }
}

#[cfg(feature = "actix-http")]
impl HeaderMapExt for actix_http::header::HeaderMap {
    fn get(&self, key: &str) -> Option<&HeaderValue> {
        self.get(key)
    }
}

pub struct PayloadHeaders {
    pub signature: Vec<u8>,
    pub message_type: MessageType,
}

pub struct ParsedHeaders<'a> {
    pub payload: PayloadHeaders,
    pub id_bytes: &'a [u8],
    pub timestamp_bytes: &'a [u8],
}

/// The [request headers](https://dev.twitch.tv/docs/eventsub/handling-webhook-events#list-of-request-headers) twitch will send.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HeaderType {
    /// `Twitch-Eventsub-Message-Id`
    Id,
    /// `Twitch-Eventsub-Message-Type`
    MessageType,
    /// `Twitch-Eventsub-Message-Signature`
    Signature,
    /// `Twitch-Eventsub-Message-Timestamp`
    Timestamp,
    /// `Twitch-Eventsub-Subscription-Version`
    SubscriptionVersion,
    /// `Twitch-Eventsub-Subscription-Type`
    SubscriptionType,
}

/// Common Errors
#[derive(Debug, thiserror::Error, Copy, Clone, PartialEq, Eq)]
pub enum InvalidHeaders {
    #[error("Missing header {0:?}")]
    Missing(HeaderType),
    #[error("Signature too short")]
    SignatureTooShort,
    #[error("Signature isn't in hexadecimal form")]
    SignatureNotHex,
    #[error("Cannot accept this version, expected: {0}")]
    VersionMismatch(&'static str),
    #[error("The timestamp is improperly formatted")]
    BadTimestamp,
    #[error("The message is too old")]
    MessageTooOld,
    #[error("This message type is not recognized")]
    BadMessageType,
    #[error("Wrong subscription type - expected {0}")]
    WrongSubscriptionType(&'static str),
}

pub fn read_eventsub_headers<M: HeaderMapExt, P: EventSubscription>(
    headers: &M,
) -> Result<ParsedHeaders<'_>, InvalidHeaders> {
    headers
        .get_subscription_type()
        .ok()
        .filter(|s| P::EVENT_TYPE.to_str().as_bytes() == s.as_bytes())
        .ok_or_else(|| InvalidHeaders::WrongSubscriptionType(P::EVENT_TYPE.to_str()))?;

    let message_type = headers.get_message_type()?;
    let signature = headers.get_signature()?;
    if signature.len() <= 7 || !signature.as_bytes().starts_with(b"sha256=") {
        return Err(InvalidHeaders::SignatureTooShort);
    }
    let signature =
        hex::decode(&signature.as_bytes()[7..]).map_err(|_| InvalidHeaders::SignatureNotHex)?;

    if headers.get_subscription_version()?.as_bytes() != P::VERSION.as_bytes() {
        return Err(InvalidHeaders::VersionMismatch(P::VERSION));
    }

    let id_header = headers.get_message_id()?;
    let timestamp_header = headers.get_message_timestamp()?;
    let timestamp = timestamp_header
        .to_str()
        .ok()
        .and_then(|h| DateTime::<Utc>::from_str(h).ok())
        .ok_or(InvalidHeaders::BadTimestamp)?;
    if Utc::now() - timestamp > Duration::minutes(10) {
        return Err(InvalidHeaders::MessageTooOld);
    }
    Ok(ParsedHeaders {
        payload: PayloadHeaders {
            signature,
            message_type,
        },
        id_bytes: id_header.as_bytes(),
        timestamp_bytes: timestamp_header.as_bytes(),
    })
}
