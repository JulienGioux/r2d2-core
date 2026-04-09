use r2d2_registry::{DatasetManifest, DatasetIdentity, DatasetMeta, TaskTypology};
use uuid::Uuid;

fn main() {
    let manifest = DatasetManifest {
        identity: DatasetIdentity {
            uuid: Uuid::new_v4(),
            name: "genesis".to_string(),
            version: "1.0.0".to_string(),
            author: Some("R2D2".to_string()),
        },
        format: TaskTypology::CausalLm,
        meta: DatasetMeta {
            size_bytes: 1024,
            samples_count: 10,
            source_corpus: "meta.json".to_string(),
        },
    };
    println!("{}", toml::to_string_pretty(&manifest).unwrap());
}
