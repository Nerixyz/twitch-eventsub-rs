//! ## Example
//!
//! Take a look at the [basic example](examples/basic_actix) as well.
//!
//! ```no_run
//! # use actix_web::{web, web::Data, App, HttpRequest, HttpResponse, HttpServer, Responder, post};
//! # use actix_web_eventsub::{guards, Config, EventsubPayload, Verification, VerifyDecodeError, types::channel::ChannelPointsCustomRewardRedemptionAddV1};
//! # use std::io;
//! struct EventsubConfig;
//!
//! impl Config for EventsubConfig {
//!     type Error = VerifyDecodeError;
//!
//!     type CheckEventIdFut = std::future::Ready<bool>;
//!
//!     fn get_secret(req: &HttpRequest) -> Result<&[u8], VerifyDecodeError> {
//!         // We put a `Data<Vec<u8>>` as `app_data` in our `App`.
//!         req.app_data::<Data<Vec<u8>>>()
//!             .map(|v| v.as_slice())
//!             .ok_or(VerifyDecodeError::NoHmacKey)
//!     }
//!
//!     fn check_event_id(req: &HttpRequest, id: &str) -> Self::CheckEventIdFut {
//!         // Here, we always handle the event
//!         // you should look at the redis example
//!         // for a more realistic implementation
//!         std::future::ready(true)
//!     }
//!
//!
//!     fn convert_error(error: VerifyDecodeError) -> Self::Error {
//!         // We're fine with the default error
//!         error
//!     }
//! }
//!
//! #[post("/eventsub")]
//! async fn event_handler(
//!     event: actix_web_eventsub::Data<ChannelPointsCustomRewardRedemptionAddV1, EventsubConfig>,
//! ) -> impl Responder {
//!     match event.payload {
//!         EventsubPayload::Verification(Verification { challenge, .. }) => {
//!             println!("Verification: {}", challenge);
//!             HttpResponse::Ok()
//!                 .content_type(mime::TEXT_PLAIN_UTF_8)
//!                 .body(challenge)
//!         }
//!         x => {
//!             println!("{:?}", x);
//!             HttpResponse::NoContent().finish()
//!         }
//!     }
//! }
//!
//! #[actix_web::main]
//! async fn main() -> io::Result<()> {
//!     let secret =
//!         Data::new(b"5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba".to_vec());
//!
//!     HttpServer::new(move || {
//!         App::new()
//!             .app_data(secret.clone())
//!             .service(event_handler)
//!     })
//!     .bind(("127.0.0.1", 8080))?
//!     .run()
//!     .await
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod extractors;
pub mod guards;

pub use extractors::eventsub::*;
pub mod types {
    //! Types for eventsub.
    pub use eventsub_common::types::*;
}
pub use eventsub_common::{EventsubPayload, Notification, Revocation, Verification};
