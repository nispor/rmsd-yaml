// SPDX-License-Identifier: Apache-2.0

static INIT_LOGGER: std::sync::Once = std::sync::Once::new();

pub(crate) fn init_logger() {
    INIT_LOGGER.call_once(|| {
        let env = env_logger::Env::default().default_filter_or("info");
        env_logger::Builder::from_env(env).init()
    });
}
