# twtich-eventsub-rs

This repository provides integrations for Twitch's [EventSub](https://dev.twitch.tv/docs/eventsub)
for [actix-web](https://actix.rs/) and [axum](https://docs.rs/axum) based on [twitch-api](https://docs.rs/twitch_api2).

## Features

* Ergonomic extractors
* Builtin verification
* Custom duplication checking (for example with redis - [actix example](actix-web-eventsub/examples/redis_actix.rs))
* Multiple types on one endpoint (actix-web only)

## [twitch-cli]

This library relies heavily on the headers Twitch is sending.
The currently released version of the [twitch-cli] doesn't send all headers correctly.

You need to compile the cli from source (currently from the [Nerixyz:fix/empty-header](https://github.com/Nerixyz/twitch-cli/tree/fix/empty-header) branch; `make build`).

## `actix-web`

### [**Basic Example**](actix-web-eventsub/examples/basic_actix.rs)

Run the example with
```
cargo r --example basic-actix
```
To test, use the [twitch-cli](#twitch-cli):

```
twitch event verify  add-redemption -F http://127.0.0.1:8080/eventsub -s 5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba
twitch event trigger add-redemption -F http://127.0.0.1:8080/eventsub -s 5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba
```

### [**Multiple Handlers**](actix-web-eventsub/examples/multiple_actix.rs)

Run the example with
```
cargo r --example multiple-actix
```
To test, use the [twitch-cli](#twitch-cli):
```
twitch event verify  update-redemption -F http://127.0.0.1:8080/eventsub -s 5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba
twitch event verify  add-redemption    -F http://127.0.0.1:8080/eventsub -s 5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba
twitch event trigger update-redemption -F http://127.0.0.1:8080/eventsub -s 5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba
twitch event trigger add-redemption    -F http://127.0.0.1:8080/eventsub -s 5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba
```

### [**Redis Duplication Checking**](actix-web-eventsub/examples/redis_actix.rs)

Run the example with
```
cargo r --example basic-actix
```
To test, use the [twitch-cli](#twitch-cli):
Trigger a regular event:
 ```
  twitch event trigger channel.channel_points_custom_reward_redemption.add -F http://127.0.0.1:8080/eventsub -s 5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba 
  ```
Copy the event-id and retrigger the evenet:
```
twitch event retrigger -i {EVENT_ID} -F http://127.0.0.1:8080/eventsub -s 5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba
```

### `axum`

### [**Basic Example**](axum-eventsub/examples/basic_axum.rs)

Run the example with
```
cargo r --example basic-axum
```
To test, use the [twitch-cli](#twitch-cli):

```
twitch event verify  add-redemption -F http://127.0.0.1:8080/eventsub -s 5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba
twitch event trigger add-redemption -F http://127.0.0.1:8080/eventsub -s 5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba
```

[twitch-cli]: https://dev.twitch.tv/docs/cli