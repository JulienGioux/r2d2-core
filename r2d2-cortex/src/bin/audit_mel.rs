use candle_transformers::models::whisper::audio::pcm_to_mel;
use candle_transformers::models::whisper::Config;
use hf_hub::{api::sync::Api, Repo, RepoType};

fn main() {
    println!("--- AUDIT DIAGNOSTIQUE DU LAYOUT MEMOIRE PCM_TO_MEL ---");

    let api = Api::new().unwrap();
    let repo = api.repo(Repo::with_revision(
        "openai/whisper-tiny".to_string(),
        RepoType::Model,
        "main".to_string(),
    ));
    let config_file = repo.get("config.json").unwrap();
    let config_str = std::fs::read_to_string(config_file).unwrap();
    let config: Config = serde_json::from_str(&config_str).unwrap();

    // Signal PCM test d'1 seconde (16000 floats)
    // On injecte un pic brutal uniquement sur les premiers 160 floats (Frame 0).
    // Les autres 15840 floats sont à zéro.
    let mut pcm = vec![0f32; 16_000];
    for item in pcm.iter_mut().take(160) {
        *item = 1.0;
    }

    let mel_filters = vec![1.0f32; 80 * 201]; // Filtres bidon à 1.0

    // On appelle pcm_to_mel. Le signal n'existe que dans le premier "hop" (Frame 0).
    let mel = pcm_to_mel(&config, &pcm, &mel_filters);

    let total_floats = mel.len();
    let frames = total_floats / 80;
    println!("Total floats générés: {}", total_floats);
    println!("Frames déduites (total / 80): {}", frames);

    // Où sont les valeurs non-zéro (ou non-faibles log-mel) ?
    // Le log-mel sur du silence donne une valeur très basse genre -10.0 ou 0 (si relu).
    // L'impulsion sur Frame 0 produira une valeur distincte.

    // Si Layout = [frames, bins]:
    // Frame 0 correspond aux indices 0 à 79.
    // Frame 1 aux indices 80 à 159.

    // Si Layout = [bins, frames]:
    // Bin 0 de la Frame 0 correspond à l'index 0.
    // Bin 0 de la Frame 1 correspond à l'index 1.
    // Bin 1 de la Frame 0 correspond à l'index `frames`.

    let val_frame0_bin1 = mel[1];
    let val_frame1_bin0 = mel[80];
    let val_frame0_bin1_alt = mel[frames];

    println!("Valeurs clés de la matrice plate:");
    println!("Index 1 : {:.4}", val_frame0_bin1);
    println!("Index 80 : {:.4}", val_frame1_bin0);
    println!("Index {} : {:.4}", frames, val_frame0_bin1_alt);

    // On analyse les variations brutes
    let mut energy_in_first_80 = 0.0;
    for item in mel.iter().take(80) {
        energy_in_first_80 += item.abs();
    }

    let mut energy_in_first_frames = 0.0;
    for i in 0..80 {
        energy_in_first_frames += mel[i * frames].abs();
    }

    println!(
        "Énergie absolue (Somme abs) dans [0..80] : {:.4}",
        energy_in_first_80
    );
    println!(
        "Énergie absolue (Somme abs) dans indices [0, frames, 2*frames... 79*frames] : {:.4}",
        energy_in_first_frames
    );

    if energy_in_first_80 > energy_in_first_frames * 2.0 {
        println!(">>> CONCLUSION : Layout est défini comme [frames, bins] ! Ordre = Row-Major (Frame per Frame).");
    } else if energy_in_first_frames > energy_in_first_80 * 2.0 {
        println!(">>> CONCLUSION : Layout est défini comme [bins, frames] ! Ordre = Column-Major.");
    } else {
        println!(">>> CONCLUSION : Indéterminé par ce test simple.");
    }
}