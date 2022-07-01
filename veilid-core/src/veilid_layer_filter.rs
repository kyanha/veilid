use super::*;
use crate::xx::*;
use tracing::level_filters::LevelFilter;
use tracing::subscriber;
use tracing_subscriber::layer;

struct VeilidLayerFilterInner {
    max_level: LevelFilter,
    ignore_list: Vec<String>,
}

#[derive(Clone)]
pub struct VeilidLayerFilter {
    inner: Arc<RwLock<VeilidLayerFilterInner>>,
}

impl VeilidLayerFilter {
    pub fn new(
        max_level: VeilidConfigLogLevel,
        ignore_list: Option<Vec<String>>,
    ) -> VeilidLayerFilter {
        Self {
            inner: Arc::new(RwLock::new(VeilidLayerFilterInner {
                max_level: max_level.to_tracing_level_filter(),
                ignore_list: ignore_list
                    .unwrap_or_else(|| DEFAULT_LOG_IGNORE_LIST.map(|x| x.to_owned()).to_vec()),
            })),
        }
    }

    pub fn max_level(&self) -> VeilidConfigLogLevel {
        let inner = self.inner.read();
        VeilidConfigLogLevel::from_tracing_level_filter(inner.max_level)
    }

    pub fn ignore_list(&self) -> Vec<String> {
        let inner = self.inner.read();
        inner.ignore_list.clone()
    }

    pub fn set_max_level(&self, level: VeilidConfigLogLevel) {
        {
            let mut inner = self.inner.write();
            inner.max_level = level.to_tracing_level_filter();
        }
        callsite::rebuild_interest_cache();
    }

    pub fn set_ignore_list(&self, ignore_list: Option<Vec<String>>) {
        {
            let mut inner = self.inner.write();
            inner.ignore_list = ignore_list
                .unwrap_or_else(|| DEFAULT_LOG_IGNORE_LIST.map(|x| x.to_owned()).to_vec());
        }
        callsite::rebuild_interest_cache();
    }
}

impl<S: tracing::Subscriber> layer::Filter<S> for VeilidLayerFilter {
    fn enabled(&self, metadata: &tracing::Metadata<'_>, _: &layer::Context<'_, S>) -> bool {
        let inner = self.inner.read();
        if *metadata.level() > inner.max_level {
            false
        } else {
            true
        }
    }

    fn callsite_enabled(
        &self,
        metadata: &'static tracing::Metadata<'static>,
    ) -> subscriber::Interest {
        let inner = self.inner.read();
        let skip = inner
            .ignore_list
            .iter()
            .any(|v| metadata.target().starts_with(&**v));
        if skip {
            subscriber::Interest::never()
        } else if *metadata.level() > inner.max_level {
            subscriber::Interest::never()
        } else {
            subscriber::Interest::always()
        }
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        let inner = self.inner.read();
        Some(inner.max_level)
    }
}
