use anyhow::{Context, Result};
use hf_hub::api::sync::Api;
use std::time::Instant;
use tokenizers::Tokenizer;

struct ModelInfo {
    id: &'static str,
    name: &'static str,
}

const MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "NousResearch/Meta-Llama-3-8B",
        name: "LLaMA-3 (BPE 128k)",
    },
    ModelInfo {
        id: "Qwen/Qwen2-7B",
        name: "Qwen-2 (BPE 152k)",
    },
    ModelInfo {
        id: "mistralai/Mistral-Nemo-Instruct-2407", // Tekken tokenizer
        name: "Mistral Tekken (~128k)",
    },
];

const PAYLOAD_JSONAI: &str = r#"{
  "vision": {
    "timestamp": "00:15:24.300",
    "scene_description": "Un homme d'une quarantaine d'années, vêtu d'un costume sombre, regarde intensément un écran holographique affichant des données boursières complexes. L'ambiance est tendue et la pièce est mal éclairée.",
    "is_fact": true,
    "confidence": 0.98
  },
  "audio": {
    "transcription": "Si les taux directeurs montent encore d'un quart de point, tout le marché va s'effondrer d'ici la fin de la journée.",
    "speaker": "SPEAKER_01",
    "emotion": "inquisiteur et anxieux",
    "is_fact": false
  },
  "consensus": "DEBATED_SYNTHESIS",
  "ontology": ["FINANCE", "CRISE", "ANTICIPATION"]
}"#;

const PAYLOAD_FR_HEAVY: &str = "En dépit des circonstances exceptionnelles qui ont frappé l'économie mondiale, l'intelligence artificielle générative a prouvé sa résilience inébranlable. L'hétérogénéité des modèles open-source soulève néanmoins un paradoxe fondamental : la démocratisation de l'accès entraîne irrémédiablement une fragmentation des standards d'ingénierie.";

fn main() -> Result<()> {
    // Initialiser le tracing
    tracing_subscriber::fmt::init();

    println!("============================================================");
    println!("⏱️ R2D2-TOKENIZER : SEMANTIC COMPRESSION BENCHMARK");
    println!("============================================================\n");

    let api = Api::new().context("Impossible d'initialiser hf-hub API. Vérifiez votre connexion.")?;
    
    // Mesures
    let text_len_json = PAYLOAD_JSONAI.len();
    let text_len_fr = PAYLOAD_FR_HEAVY.len();

    println!("Payloads de test :");
    println!("- JSONAI V3 : {} octets", text_len_json);
    println!("- Français Lourd : {} octets\n", text_len_fr);

    for model in MODELS {
        println!("------------------------------------------------------------");
        println!("🚀 PULLING/LOADING : {}", model.name);
        let start_load = Instant::now();
        
        let repo = api.model(model.id.to_string());
        
        let tokenizer_filename = match repo.get("tokenizer.json") {
            Ok(file) => file,
            Err(e) => {
                println!("❌ Erreur de téléchargement pour {} : {}", model.id, e);
                println!("(Si vous voyez une erreur 401 Unauthorized, c'est un modèle Gated nécessitant HF_TOKEN. Assurez-vous d'avoir fait `huggingface-cli login`)");
                continue;
            }
        };

        let tokenizer = Tokenizer::from_file(&tokenizer_filename).unwrap();
        let elapsed_load = start_load.elapsed();

        println!("✅ Chargeur prêt en {:?}", elapsed_load);

        // Evaluation JSON
        let encoding_json = tokenizer.encode(PAYLOAD_JSONAI, true).unwrap();
        let tokens_json = encoding_json.get_ids().len();
        let ratio_json = text_len_json as f64 / tokens_json as f64;
        
        // Evaluation FR
        let encoding_fr = tokenizer.encode(PAYLOAD_FR_HEAVY, true).unwrap();
        let tokens_fr = encoding_fr.get_ids().len();
        let ratio_fr = text_len_fr as f64 / tokens_fr as f64;

        println!("📊 Résultats de Compression :");
        println!("   [JSONAI] Tokens : {} | Compression : {:.2} octets/token", tokens_json, ratio_json);
        println!("   [FR]     Tokens : {} | Compression : {:.2} octets/token", tokens_fr, ratio_fr);
    }

    println!("\n============================================================");
    println!("🎯 CONCLUSION ATTENDUE : Plus le ratio (octets/token) est élevé,");
    println!("plus le dictionnaire est dense et nous permet de traiter de longues");
    println!("vidéos en économisant de la VRAM (Context Window).");
    println!("============================================================");

    Ok(())
}
