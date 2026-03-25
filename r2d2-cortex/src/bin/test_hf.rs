use hf_hub::api::tokio::Api;
use hf_hub::{Repo, RepoType};

#[tokio::main]
async fn main() {
    let api = Api::new().unwrap();
    let repo = api.repo(Repo::with_revision(
        "intfloat/multilingual-e5-large-instruct".to_string(),
        RepoType::Model,
        "main".to_string(),
    ));

    println!("Testing tokenizer.json...");
    match repo.get("tokenizer.json").await {
        Ok(path) => println!("Success: {:?}", path),
        Err(e) => println!("Error tokenizer: {:?}", e),
    }

    println!("Testing config.json...");
    match repo.get("config.json").await {
        Ok(path) => println!("Success: {:?}", path),
        Err(e) => println!("Error config: {:?}", e),
    }
}