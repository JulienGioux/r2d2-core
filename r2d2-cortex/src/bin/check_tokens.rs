use hf_hub::{api::sync::Api, Repo, RepoType};
use tokenizers::Tokenizer;

fn main() {
    println!("--- AUDIT DES TOKENS WHISPER-TINY ---");

    let api = Api::new().unwrap();
    let repo = api.repo(Repo::with_revision(
        "openai/whisper-large-v3-turbo".to_string(),
        RepoType::Model,
        "main".to_string(),
    ));
    let tokenizer_file = repo.get("tokenizer.json").unwrap();
    let tokenizer = Tokenizer::from_file(&tokenizer_file).unwrap();

    // 1. Décodage de nos tokens hardcodés
    let tokens = vec![50258, 50265, 50359, 50363];
    println!("Décodage de [50258, 50265, 50359, 50363]:");
    // Avec special tokens
    let decoded_with_special = tokenizer.decode(&tokens, false).unwrap();
    println!("  -> Avec Special: '{}'", decoded_with_special);
    // Sans special tokens
    let decoded_without_special = tokenizer.decode(&tokens, true).unwrap();
    println!("  -> Sans Special: '{}'", decoded_without_special);

    // 2. Encodage des tags texte pour voir leurs VRAIS IDs
    let special_tags = vec![
        "<|startoftranscript|>",
        "<|fr|>",
        "<|transcribe|>",
        "<|notimestamps|>",
    ];
    for tag in special_tags {
        let encoding = tokenizer.encode(tag, true).unwrap();
        println!("Encodage de '{}' -> {:?}", tag, encoding.get_ids());
    }

    // 3. Décodage de la spirale hallucinatoire détectée (18, 126)
    let spiral = vec![18, 126, 18, 126, 18, 126];
    let decoded_spiral = tokenizer.decode(&spiral, true).unwrap();
    println!(
        "Décodage de [18, 126, 18, 126, 18, 126] -> '{}'",
        decoded_spiral
    );

    // 4. Décodage du token unique 50257 (EOT)
    let eot = tokenizer.decode(&[50257], false).unwrap();
    println!("Token 50257 désigne -> '{}'", eot);
}