use r2d2_registry::{ModelFamily, ModelId, ModelRegistry, QuantizationLevel, RegistryError};
use std::fs;
use std::io::Write;
use tempfile::tempdir;
use uuid::Uuid;

#[tokio::test]
async fn test_integration_full_registry_lifecycle() {
    let dir = tempdir().unwrap();
    let registry = ModelRegistry::new(dir.path());

    // 1. Démarrage à vide
    assert_eq!(registry.catalog().await.len(), 0);

    // 2. Création d'une structure malformée (Manque manifest)
    let invalid_dir = dir.path().join("bitmamba").join("ghost_model");
    fs::create_dir_all(&invalid_dir).unwrap();
    assert_eq!(registry.catalog().await.len(), 0);

    // 3. Création d'un modèle valide
    let valid_dir = dir.path().join("embedding").join("nomic-v1");
    fs::create_dir_all(&valid_dir).unwrap();

    let valid_uuid = Uuid::new_v4();
    let manifest_toml = format!(
        r#"
format = "contrastive_embedding"

[identity]
uuid = "{}"
name = "Nomic-Test"
version = "1.5.0"
family = "embedding"

[topology]
architecture = "Bert"
quantization = "fp32"

[metrics]
optimal_tasks = ["RAG"]
"#,
        valid_uuid
    );

    let mut file = fs::File::create(valid_dir.join("manifest.toml")).unwrap();
    file.write_all(manifest_toml.as_bytes()).unwrap();

    // 4. Test d'indexation
    let catalog = registry.catalog().await;
    assert_eq!(catalog.len(), 1);

    let manifest = &catalog[0].1;
    assert_eq!(manifest.identity.name, ModelId("Nomic-Test".to_string()));
    assert_eq!(manifest.topology.quantization, QuantizationLevel::Fp32);

    // 5. Test Finders
    let found_by_uuid = registry
        .find_by_uuid(&valid_uuid)
        .await
        .expect("UUID introuvable");
    assert_eq!(found_by_uuid.1.identity.name.0, "Nomic-Test");

    let found_by_name = registry
        .find_by_name(&ModelId("Nomic-Test".to_string()))
        .await
        .expect("Name introuvable");
    assert_eq!(found_by_name.1.identity.family, ModelFamily::Embedding);
}

#[tokio::test]
async fn test_integration_manifest_deserialization_safeguards() {
    let dir = tempdir().unwrap();
    let model_dir = dir.path().join("llama").join("hacked");
    fs::create_dir_all(&model_dir).unwrap();

    // Test: Variante d'énumération inexistante (Zero-Trust Validation)
    let bad_manifest = r#"
[identity]
uuid = "a3bc9dfc-ab84-489e-8c34-eb54e3d3b76a"
name = "Hacked-Model"
version = "1"
family = "hacker_family" # N'EXISTE PAS DANS L'ENUM

[topology]
architecture = "Malware"
quantization = "fp32"
"#;
    let mut file = fs::File::create(model_dir.join("manifest.toml")).unwrap();
    file.write_all(bad_manifest.as_bytes()).unwrap();

    let registry = ModelRegistry::new(dir.path());
    let catalog = registry.catalog().await;

    // Le parseur TOML refuse la deserialisation de l'enum invalide -> le modèle est ignoré silencieusement (Zéro Panique).
    assert_eq!(catalog.len(), 0);

    // Vérif explicite de l'erreur
    let err = registry.load_manifest(&model_dir).await.unwrap_err();
    assert!(matches!(err, RegistryError::ParseToml(_)));
}
