use candle_transformers::models::whisper::audio::pcm_to_mel;
use candle_transformers::models::whisper::Config;
use hf_hub::{api::sync::Api, Repo, RepoType};

fn main() {
    println!("--- DEBUG WHISPER GEOMETRY ---");
    let api = Api::new().unwrap();
    let repo = api.repo(Repo::with_revision(
        "openai/whisper-tiny".to_string(),
        RepoType::Model,
        "main".to_string(),
    ));
    let config_file = repo.get("config.json").unwrap();

    let config_str = std::fs::read_to_string(config_file).unwrap();
    let config: Config = serde_json::from_str(&config_str).unwrap();

    println!("num_mel_bins: {}", config.num_mel_bins);
    println!("max_source_positions: {}", config.max_source_positions);

    let pcm = vec![0f32; 480_000];
    let mel_filters = vec![0f32; config.num_mel_bins * 201];
    let mel = pcm_to_mel(&config, &pcm, &mel_filters);

    println!("mel_filters size: {}", mel_filters.len());
    println!("output mel size: {}", mel.len());
    println!(
        "output mel / bins = frames: {}",
        mel.len() as f32 / config.num_mel_bins as f32
    );
    println!("output mel / 3000 = bins: {}", mel.len() as f32 / 3000.0);
}


