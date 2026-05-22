use std::{collections::BTreeMap, sync::Arc};

use crate::model::{
    BaselineDirectionModel, StrategyModel, BASELINE_DIRECTION_KEY, BASELINE_DIRECTION_VERSION,
};

#[derive(Clone, Default)]
pub struct ModelRegistry {
    models: BTreeMap<(String, String), Arc<dyn StrategyModel>>,
}

impl ModelRegistry {
    pub fn built_ins() -> Self {
        let mut registry = Self::default();
        registry.register(Arc::new(BaselineDirectionModel::btc5m_default()));
        registry
    }

    pub fn register(&mut self, model: Arc<dyn StrategyModel>) {
        self.models.insert(
            (model.key().to_string(), model.version().to_string()),
            model,
        );
    }

    pub fn get(&self, model_key: &str, version: &str) -> Option<Arc<dyn StrategyModel>> {
        self.models
            .get(&(model_key.to_string(), version.to_string()))
            .cloned()
    }

    pub fn model_versions(&self) -> Vec<(String, String)> {
        self.models.keys().cloned().collect()
    }
}

impl From<BaselineDirectionModel> for ModelRegistry {
    fn from(model: BaselineDirectionModel) -> Self {
        let mut registry = Self::default();
        registry.register(Arc::new(model));
        registry
    }
}

pub fn built_in_model_version() -> (&'static str, &'static str) {
    (BASELINE_DIRECTION_KEY, BASELINE_DIRECTION_VERSION)
}
