use std::future::ready;

use actix_web::{post, App, HttpResponse, Responder};
use actix_web_eventsub::{guards, Config};
use eventsub_common::{
    types::{
        channel::{
            ChannelPointsCustomRewardRedemptionAddV1, ChannelPointsCustomRewardRedemptionUpdateV1,
        },
        EventType,
    },
    EventsubPayload, Verification,
};
use util::SecretConfig;

use crate::util::{BaseSecret, SecondSecret};
mod util;

struct TestConfig<T> {
    _marker: std::marker::PhantomData<T>,
}

impl<T: SecretConfig> Config for TestConfig<T> {
    type Error = actix_web_eventsub::VerifyDecodeError;

    type CheckEventIdFut = std::future::Ready<bool>;

    fn get_secret(_: &actix_web::HttpRequest) -> Result<&[u8], Self::Error> {
        Ok(T::secret())
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
    event: actix_web_eventsub::Data<
        ChannelPointsCustomRewardRedemptionAddV1,
        TestConfig<BaseSecret>,
    >,
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

#[post(
    "/guarded",
    guard = "guards::event_type_fn::<ChannelPointsCustomRewardRedemptionAddV1>"
)]
async fn guarded_redemption_add(
    event: actix_web_eventsub::Data<
        ChannelPointsCustomRewardRedemptionAddV1,
        TestConfig<BaseSecret>,
    >,
) -> impl Responder {
    match event.payload {
        EventsubPayload::Verification(Verification {
            challenge,
            subscription,
        }) => {
            assert_eq!(
                subscription.type_,
                EventType::ChannelPointsCustomRewardRedemptionAdd
            );
            HttpResponse::Ok()
                .content_type(mime::TEXT_PLAIN_UTF_8)
                .body(challenge)
        }
        x => {
            panic!("Received unexpected payload: {x:?}");
        }
    }
}

#[post(
    "/guarded",
    guard = "guards::event_type_fn::<ChannelPointsCustomRewardRedemptionUpdateV1>"
)]
async fn guarded_redemption_update(
    event: actix_web_eventsub::Data<
        ChannelPointsCustomRewardRedemptionUpdateV1,
        TestConfig<SecondSecret>,
    >,
) -> impl Responder {
    match event.payload {
        EventsubPayload::Verification(Verification {
            challenge,
            subscription,
        }) => {
            assert_eq!(
                subscription.type_,
                EventType::ChannelPointsCustomRewardRedemptionUpdate
            );
            HttpResponse::Ok()
                .content_type(mime::TEXT_PLAIN_UTF_8)
                .body(challenge)
        }
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

#[actix_web::test]
async fn guards() -> anyhow::Result<()> {
    let srv = actix_test::start(|| {
        App::new()
            .service(guarded_redemption_add)
            .service(guarded_redemption_update)
    });

    util::twitch_cli(|cmd| {
        cmd.arg("verify")
            .arg("channel.channel_points_custom_reward_redemption.add")
            .arg("-F")
            .arg(&format!("http://{}/guarded", srv.addr()))
            .arg("-s")
            .arg(std::str::from_utf8(util::SECRET).unwrap());
    })
    .await;

    util::twitch_cli(|cmd| {
        cmd.arg("verify")
            .arg("channel.channel_points_custom_reward_redemption.update")
            .arg("-F")
            .arg(&format!("http://{}/guarded", srv.addr()))
            .arg("-s")
            .arg(std::str::from_utf8(util::SECRET2).unwrap());
    })
    .await;

    srv.stop().await;

    Ok(())
}
