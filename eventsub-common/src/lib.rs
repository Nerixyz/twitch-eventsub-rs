use http::HeaderValue;
use serde::{Deserialize, Serialize};
use types::{EventSubSubscription, EventSubscription};

/// The eventsub payload sent by twitch.
/// It may be a [`Verification`], [`Notification`] or [`Revocation`].
#[derive(Debug, Clone, PartialEq)]
pub enum EventsubPayload<T> {
    /// See [`Verification`]
    Verification(Verification),
    /// See [`Notification`]
    Notification(Notification<T>),
    /// See [`Revocation`]
    Revocation(Revocation),
}

/// A verification payload.
/// The server must respond to this payload with the `challenge` string as text.
///
/// Take a look at the examples and use the [twitch-cli](https://github.com/twitchdev/twitch-cli) to verify your implementation.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Verification {
    /// The challenge value
    pub challenge: String,
    /// The current subscription
    pub subscription: EventSubSubscription,
}

/// A notification payload.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Notification<T> {
    /// The event's data
    #[serde(bound = "T: EventSubscription")]
    pub event: T,
    /// The current subscription
    pub subscription: EventSubSubscription,
}

/// A revocation payload.
///
/// Twitch will no longer send events for this subscription.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Revocation {
    /// The revoked subscription
    pub subscription: EventSubSubscription,
}

/// Internal hint for the target message type when deserializing.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MessageType {
    /// A notification is sent.
    Notification,
    /// A verification is sent.
    Verification,
    /// A revocation is sent.
    Revocation,
}

impl TryFrom<&HeaderValue> for MessageType {
    type Error = ();

    fn try_from(value: &HeaderValue) -> Result<Self, Self::Error> {
        match value.to_str() {
            Ok("notification") => Ok(Self::Notification),
            Ok("webhook_callback_verification") => Ok(Self::Verification),
            Ok("revocation") => Ok(Self::Revocation),
            _ => Err(()),
        }
    }
}

pub mod headers;
pub mod types {
    pub use twitch_api::eventsub::*;
}
