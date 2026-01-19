// SPDX-License-Identifier: Apache-2.0

static INIT_LOGGER: std::sync::Once = std::sync::Once::new();

pub(crate) fn init_logger() {
    INIT_LOGGER.call_once(|| {
        env_logger::builder()
            .filter_level(log::LevelFilter::Trace)
            .init()
    });
}
