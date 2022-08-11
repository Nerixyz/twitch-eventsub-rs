use axum::{http::Extensions, routing::post, Extension, Router};
use axum_eventsub::{
    types::channel::ChannelPointsCustomRewardRedemptionAddV1, Verification, VerifyDecodeError,
};
use eventsub_common::EventsubPayload;
use std::{borrow::Cow, sync::Arc};

struct EventsubConfig;

impl axum_eventsub::Config for EventsubConfig {
    type Rejection = VerifyDecodeError;

    fn get_secret(ext: &Extensions) -> Option<&[u8]> {
        ext.get::<Arc<Vec<u8>>>().map(|v| v.as_slice())
    }

    fn convert_error(error: VerifyDecodeError) -> Self::Rejection {
        error
    }
}

async fn eventsub(
    data: axum_eventsub::Data<ChannelPointsCustomRewardRedemptionAddV1, EventsubConfig>,
) -> Cow<'static, str> {
    match data.payload {
        EventsubPayload::Verification(Verification { challenge, .. }) => challenge.into(),
        x => {
            println!("{x:?}");
            "".into()
        }
    }
}

/// Run the example with
/// cargo r --example basic-axum
/// To test, use the twitch-cli:
/// twitch event verify  add-redemption -F http://127.0.0.1:8080/eventsub -s 5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba
/// twitch event trigger add-redemption -F http://127.0.0.1:8080/eventsub -s 5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba
///
/// Note that you need to build the twitch-cli from source, because the currently released version
/// has bugs regarding some headers.
#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/eventsub", post(eventsub))
        // We don't hex decode here, to match twitch-cli behavior
        .layer(Extension(Arc::new(
            b"5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba".to_vec(),
        )));

    // run it with hyper on localhost:8080
    axum::Server::bind(&"0.0.0.0:8080".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
