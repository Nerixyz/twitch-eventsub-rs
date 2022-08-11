//! This module contains useful guards when dealing with `EventSub` requests

use crate::types::EventSubscription;
use actix_web::guard::{Guard, GuardContext};
use eventsub_common::headers;
use std::marker::PhantomData;

/// Guard for an eventsub event.
pub struct EventTypeGuard<T> {
    _event: PhantomData<T>,
}

/// Create a guard for an eventsub event.
/// This guard will check the subscription type and version.
///
/// ```
/// # use actix_web::{Responder, web};
/// # use actix_web_eventsub::{guards, types::channel::ChannelPointsCustomRewardRedemptionAddV1};
/// #
/// # async fn event_handler() -> impl Responder { "" }
/// fn configure(config: &mut web::ServiceConfig) {
/// config.route(
///         "/eventsub",
///         web::post()
///             .guard(guards::event_type::<ChannelPointsCustomRewardRedemptionAddV1>())
///             .to(event_handler),
///     );
/// }
/// # fn main() {}
/// ```
#[must_use]
pub fn event_type<T: EventSubscription>() -> EventTypeGuard<T> {
    EventTypeGuard {
        _event: PhantomData,
    }
}

impl<T: EventSubscription> Guard for EventTypeGuard<T> {
    fn check(&self, ctx: &GuardContext) -> bool {
        event_type_fn::<T>(ctx)
    }
}

/// A guard for an eventsub event.
/// This guard will check the subscription type and version.
///
/// Use this guard in a [`guard_fn`](actix_web::guards::guard_fn).
///
/// ```
/// # use actix_web::{Responder, web, HttpRequest, HttpResponse};
/// # use actix_web_eventsub::{guards,VerifyDecodeError, EventsubPayload, Config, types::channel::ChannelPointsCustomRewardRedemptionAddV1};
/// #
/// # struct EventsubConfig;
/// #
/// # /// Same implementation as in the basic example
/// # impl Config for EventsubConfig {
/// #     type Error = VerifyDecodeError;
/// #     type CheckEventIdFut = std::future::Ready<bool>;
/// #
/// #     fn get_secret(req: &HttpRequest) -> Result<&[u8], Self::Error> {
/// #        Err(VerifyDecodeError::NoHmacKey)
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
/// #[post(
///      "/eventsub",
///      guard = "guards::event_type_fn::<ChannelPointsCustomRewardRedemptionAddV1>"
///  )]
/// async fn event_handler(
///     event: actix_web_eventsub::Data<ChannelPointsCustomRewardRedemptionAddV1, EventsubConfig>,
/// ) -> impl Responder {
///    println!("Add payload: {:?}", event.payload);
///    HttpResponse::NoContent().finish()
/// }
/// fn configure(config: &mut web::ServiceConfig) {
///     config.service(event_handler);
/// }
/// # fn main() {}
/// ```
#[must_use]
pub fn event_type_fn<T: EventSubscription>(ctx: &GuardContext) -> bool {
    match (
        ctx.head().headers.get(headers::SUBSCRIPTION_TYPE),
        ctx.head().headers.get(headers::SUBSCRIPTION_VERSION),
    ) {
        (Some(sub_type), Some(sub_version)) => {
            sub_version.as_bytes() == T::VERSION.as_bytes()
                && sub_type.as_bytes() == T::EVENT_TYPE.to_str().as_bytes()
        }
        _ => false,
    }
}
