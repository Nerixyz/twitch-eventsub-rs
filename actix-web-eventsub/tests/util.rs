use tokio::process::Command;

pub const SECRET: &[u8] = b"5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328ba";
// Second secret used to separate different routes
pub const SECRET2: &[u8] = b"5f5f121fc807a21bab4209b2f34e90932778f12c099ca3ca17ee00afd0b328bb";

pub trait SecretConfig {
    fn secret() -> &'static [u8];
}

macro_rules! make_secret_configs {
    ($($name:ident => $evaluate:expr,)*) => {
        $(
            pub struct $name;
            impl SecretConfig for $name {
                fn secret() -> &'static [u8] {
                    $evaluate
                }
            }
        )*
    };
}

make_secret_configs!(
    BaseSecret => SECRET,
    SecondSecret => SECRET2,
);

pub async fn twitch_cli(args: impl FnOnce(&mut Command)) {
    let mut cmd = Command::new("twitch");
    cmd.arg("event");
    args(&mut cmd);
    let output = cmd.output().await.expect("twitch-cli should run");
    if !output.status.success() {
        panic!("{cmd:?} exited with {output:?}");
    }
    dbg!((&cmd, &output));
}
