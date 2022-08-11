use actix_web::{post, web::Data, App, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_eventsub::{
    types::channel::ChannelPointsCustomRewardRedemptionAddV1, Config, EventsubPayload,
    Verification, VerifyDecodeError,
};
use futures_util::{future, FutureExt};
use std::{
    future::{ready, Ready},
    io,
};

struct EventsubConfig;

impl Config for EventsubConfig {
    type Error = VerifyDecodeError;
    type CheckEventIdFut = future::Either<Ready<bool>, future::BoxFuture<'static, bool>>;

    fn get_secret(req: &HttpRequest) -> Result<&[u8], Self::Error> {
        req.app_data::<Data<Vec<u8>>>()
            .map(|v| v.as_slice())
            .ok_or(VerifyDecodeError::NoHmacKey)
    }

    fn check_event_id(req: &HttpRequest, id: &str) -> Self::CheckEventIdFut {
        let pool = match req.app_data::<deadpool_redis::Pool>() {
            Some(pool) => pool.clone(),
            None => {
                eprintln!("Cannot get Pool from app-data");
                return future::Either::Left(ready(false));
            }
        };
        let key = format!("eventsub:{id}");
        future::Either::Right(
            async move {
                let mut conn = match pool.get().await {
                    Ok(conn) => conn,
                    Err(e) => {
                        eprintln!("Cannot get connection: {e}");
                        return false;
                    }
                };
                match deadpool_redis::redis::cmd("SET")
                    .arg(&key)
                    .arg(1)
                    .arg("NX")
                    .arg("EX")
                    .arg(15 * 60)
                    .query_async(&mut conn)
                    .await
                {
                    Err(e) => {
                        eprintln!("Couldn't set event-id key: {e}");
                        false
                    }
                    Ok(deadpool_redis::redis::Value::Nil) => false,
                    Ok(deadpool_redis::redis::Value::Okay) => true,
                    Ok(v) => {
                        eprintln!("Unexpected reply: {v:?}");
                        false
                    }
                }
            }
            .boxed(),
        )
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
/// (1) twitch event trigger channel.channel_points_custom_reward_redemption.add -F http://127.0.0.1:8080/eventsub -s 5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba
/// (2) Copy the event-id
/// (3) twitch event retrigger -i {EVENT_ID} -F http://127.0.0.1:8080/eventsub -s 5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba
///
/// Note that you need to build the twitch-cli from source, because the currently released version
/// has bugs regarding some headers.
#[actix_web::main]
async fn main() -> io::Result<()> {
    // We don't hex decode here, to match twitch-cli behavior
    let secret =
        Data::new(b"5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba".to_vec());
    let redis_pool = deadpool_redis::Config::from_url("redis://127.0.0.1/")
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .unwrap();

    HttpServer::new(move || {
        App::new()
            .app_data(secret.clone())
            .app_data(redis_pool.clone())
            .service(event_handler)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
