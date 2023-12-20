use std::future::ready;

use actix_web::{post, web::Data, App, HttpResponse, Responder};
use actix_web_eventsub::Config;
use eventsub_common::{
    types::channel::ChannelPointsCustomRewardRedemptionAddV1, EventsubPayload, Verification,
};
mod util;

struct TestConfig;
impl Config for TestConfig {
    type Error = actix_web_eventsub::VerifyDecodeError;

    type CheckEventIdFut = std::future::Ready<bool>;

    fn get_secret(_: &actix_web::HttpRequest) -> Result<&[u8], Self::Error> {
        Ok(util::SECRET)
    }

    fn check_event_id(_req: &actix_web::HttpRequest, _id: &str) -> Self::CheckEventIdFut {
        ready(true)
    }

    fn convert_error(error: actix_web_eventsub::VerifyDecodeError) -> Self::Error {
        error
    }
}

#[post("/eventsub")]
async fn event_handler(
    event: actix_web_eventsub::Data<ChannelPointsCustomRewardRedemptionAddV1, TestConfig>,
) -> impl Responder {
    match event.payload {
        EventsubPayload::Verification(Verification { challenge, .. }) => HttpResponse::Ok()
            .content_type(mime::TEXT_PLAIN_UTF_8)
            .body(challenge),
        x => {
            panic!("Received unexpected payload: {x:?}");
        }
    }
}

#[actix_web::test]
async fn basic() -> anyhow::Result<()> {
    let srv = actix_test::start(|| App::new().service(event_handler));

    util::twitch_cli(|cmd| {
        cmd.arg("verify")
            .arg("channel.channel_points_custom_reward_redemption.add")
            .arg("-F")
            .arg(&format!("http://{}/eventsub", srv.addr()))
            .arg("-s")
            .arg(std::str::from_utf8(util::SECRET).unwrap());
    })
    .await;

    srv.stop().await;

    Ok(())
}
