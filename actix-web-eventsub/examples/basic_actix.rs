use actix_web::{post, web::Data, App, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_eventsub::{
    types::channel::ChannelPointsCustomRewardRedemptionAddV1, Config, EventsubPayload,
    Verification, VerifyDecodeError,
};
use std::{
    future::{ready, Ready},
    io,
};

struct EventsubConfig;

impl Config for EventsubConfig {
    type Error = VerifyDecodeError;
    type CheckEventIdFut = Ready<bool>;

    fn get_secret(req: &HttpRequest) -> Result<&[u8], Self::Error> {
        req.app_data::<Data<Vec<u8>>>()
            .map(|v| v.as_slice())
            .ok_or(VerifyDecodeError::NoHmacKey)
    }

    fn check_event_id(_req: &HttpRequest, _id: &str) -> Self::CheckEventIdFut {
        // Here, we always handle the event
        // you should look at the redis example
        // for a more realistic implementation
        ready(true)
    }

    fn convert_error(error: VerifyDecodeError) -> Self::Error {
        // We're fine with the default error
        error
    }
}

#[post("/eventsub")]
async fn event_handler(
    event: actix_web_eventsub::Data<ChannelPointsCustomRewardRedemptionAddV1, EventsubConfig>,
) -> impl Responder {
    match event.payload {
        EventsubPayload::Verification(Verification { challenge, .. }) => {
            println!("Verification: {}", challenge);
            HttpResponse::Ok()
                .content_type(mime::TEXT_PLAIN_UTF_8)
                .body(challenge)
        }
        x => {
            println!("{:?}", x);
            HttpResponse::NoContent().finish()
        }
    }
}

/// Run the example with
/// cargo r --example basic-actix
/// To test, use the twitch-cli:
/// twitch event verify  add-redemption -F http://127.0.0.1:8080/eventsub -s 5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba
/// twitch event trigger add-redemption -F http://127.0.0.1:8080/eventsub -s 5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba
///
/// Note that you need to build the twitch-cli from source, because the currently released version
/// has bugs regarding some headers.
#[actix_web::main]
async fn main() -> io::Result<()> {
    // We don't hex decode here, to match twitch-cli behavior
    let secret =
        Data::new(b"5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba".to_vec());

    HttpServer::new(move || App::new().app_data(secret.clone()).service(event_handler))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
