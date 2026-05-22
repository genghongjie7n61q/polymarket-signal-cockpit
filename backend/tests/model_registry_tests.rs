use polymarket_backend::model::{ModelRegistry, BASELINE_DIRECTION_KEY, BASELINE_DIRECTION_VERSION};

#[test]
fn built_in_registry_resolves_baseline_assignment() {
    let registry = ModelRegistry::built_ins();
    let model = registry
        .get(BASELINE_DIRECTION_KEY, BASELINE_DIRECTION_VERSION)
        .expect("baseline model should be registered");

    assert_eq!(model.key(), BASELINE_DIRECTION_KEY);
    assert_eq!(model.version(), BASELINE_DIRECTION_VERSION);
}

#[test]
fn registry_rejects_unknown_model_version() {
    let registry = ModelRegistry::built_ins();

    assert!(registry.get(BASELINE_DIRECTION_KEY, "9.9.9").is_none());
}

#[test]
fn registry_lists_registered_versions_in_stable_order() {
    let registry = ModelRegistry::built_ins();

    assert_eq!(
        registry.model_versions(),
        vec![(BASELINE_DIRECTION_KEY.to_string(), BASELINE_DIRECTION_VERSION.to_string())]
    );
}
