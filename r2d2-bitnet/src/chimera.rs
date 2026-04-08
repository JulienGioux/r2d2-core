use crate::hadamard::HadamardLayer;
use crate::moe::SparseMoe;
use crate::ssm::SsmBlock;
use candle_core::{Device, IndexOp, Module, Result, Tensor, D};
use candle_nn::{embedding, linear_no_bias, Embedding, Linear, VarBuilder};
use tracing::{info, instrument};

/// ⚙️ Paramétrage du Modèle R2D2-Chimera
#[derive(Debug, Clone)]
pub struct ChimeraConfig {
    pub hidden_size: usize,
    pub num_hidden_layers: usize,
    pub num_experts: usize,
    pub top_k: usize,
    pub vocab_size: usize,
}

impl ChimeraConfig {
    /// Modèle réduit pour les tests CI et le Mocking VRAM (Évite d'exploser la RAM)
    pub fn reduced() -> Self {
        Self {
            hidden_size: 256,
            num_hidden_layers: 2,
            num_experts: 4,
            top_k: 2,
            vocab_size: 32000,
        }
    }

    pub fn b1_58_3b() -> Self {
        Self {
            hidden_size: 4096, // Typique Llama-3B like
            num_hidden_layers: 32,
            num_experts: 8,
            top_k: 2,
            vocab_size: 128256, // Llama-3 128k
        }
    }

    /// Détection Plug&Play Dynamique de l'Environnement (VRAM/RAM)
    /// Lit `/proc/meminfo` sous Linux ou WSL pour adapter le profil. Implémenté de manière thread-safe (I/O block évité par OnceLock).
    pub fn auto() -> Self {
        static CACHED_RAM_GB: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

        let total_ram_gb = *CACHED_RAM_GB.get_or_init(|| {
            let mut mem = 16; // Assumption par défaut acceptable
            #[cfg(target_os = "linux")]
            {
                if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
                    for line in content.lines() {
                        if line.starts_with("MemTotal:") {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if parts.len() >= 2 {
                                if let Ok(kb) = parts[1].parse::<usize>() {
                                    mem = kb / (1024 * 1024);
                                }
                            }
                            break;
                        }
                    }
                }
            }
            mem
        });

        // 12 Go est la barrière critique de RAM d'une RTX 3060 (ou d'une partition WSL resserrée)
        if total_ram_gb <= 14 {
            tracing::info!("⚠️  [Chimera Forge] Système de Diagnostic: RAM Hôte/WSL plafonnée à {} Go. Activation du profil de Survie [REDUCED] pour prévenir l'OOM.", total_ram_gb);
            Self::reduced()
        } else {
            tracing::info!("🚀 [Chimera Forge] Système de Diagnostic: Confort Mémoire garanti ({} Go). Boot du modèle massique [b1.58_3B].", total_ram_gb);
            Self::b1_58_3b()
        }
    }
}

/// 🏛️ `ChimeraTransformerBlock` (Hadamard -> SSM -> MoE)
pub struct ChimeraBlock {
    pub hadamard: HadamardLayer,
    pub ssm: SsmBlock,
    pub moe: SparseMoe,
}

impl ChimeraBlock {
    pub fn forward(&self, x: &Tensor, prev_state: Option<Vec<f32>>) -> Result<(Tensor, Vec<f32>)> {
        // 1. Stabilisation Quantique
        let h1 = self.hadamard.forward(x)?;

        // 2. Continuous State Space (BitMamba) MLGRU
        let (ssm_out, new_state) = self.ssm.forward_scan(&h1, prev_state)?;

        // 3. Routage dynamique
        let out = self.moe.forward(&ssm_out)?;

        // Connexion résiduelle
        let final_out = out.broadcast_add(x)?;

        Ok((final_out, new_state))
    }
}

/// 🧠 Le Modèle de Langage Complet R2D2-Chimera
pub struct ChimeraModel {
    pub config: ChimeraConfig,
    layers: Vec<ChimeraBlock>,
    /// Vraie table d'Embedding (Lexique vers Vecteur Dense)
    pub embed: Embedding,
    /// Vraie Tête de Génération Linguistique
    pub lm_head: Linear,
}

impl ChimeraModel {
    /// 🚀 Initalisation "QAT-Scratch" avec VarBuilder (Haute-Précision FP32 latente)
    /// Utilisera XavierNormal pour les matrices denses.
    #[instrument(skip_all)]
    pub fn new_qat(config: &ChimeraConfig, vb: VarBuilder) -> Result<Self> {
        info!(
            "Instanciation de la topologie Inférence ChimeraModel (QAT-Scratch) [{}] layers",
            config.num_hidden_layers
        );

        let mut layers = Vec::with_capacity(config.num_hidden_layers);

        for i in 0..config.num_hidden_layers {
            let vb_layer = vb.pp(format!("layer_{i}"));

            let hadamard = HadamardLayer::new(config.hidden_size);

            // SSM & MoE prendront le VarBuilder pour initialiser proprement (HiPPO & Xavier)
            let ssm = SsmBlock::new_qat(config.hidden_size, vb_layer.pp("ssm"))?;
            let moe = SparseMoe::new_qat(
                config.hidden_size,
                config.num_experts,
                config.top_k,
                vb_layer.pp("moe"),
            )?;

            layers.push(ChimeraBlock { hadamard, ssm, moe });
        }

        let embed = embedding(config.vocab_size, config.hidden_size, vb.pp("embed"))?;
        let lm_head = linear_no_bias(config.hidden_size, config.vocab_size, vb.pp("lm_head"))?;

        Ok(Self {
            config: config.clone(),
            layers,
            embed,
            lm_head,
        })
    }

    /// Propagation avant sur une séquence de tokens
    #[instrument(skip_all)]
    pub fn forward(
        &self,
        tokens: &Tensor,
        mut prev_states: Option<Vec<Vec<f32>>>,
    ) -> Result<(Tensor, Vec<Vec<f32>>)> {
        let mut x = self.embed.forward(tokens)?;

        let mut next_states = Vec::with_capacity(self.layers.len());

        for (i, layer) in self.layers.iter().enumerate() {
            // Take ownership of the layer state to recycle its allocation
            let prev_s = if let Some(ref mut states_vec) = prev_states {
                if i < states_vec.len() {
                    let extracted = std::mem::take(&mut states_vec[i]);
                    Some(extracted)
                } else {
                    None
                }
            } else {
                None
            };

            let (out_x, new_s) = layer.forward(&x, prev_s)?;
            x = out_x;
            next_states.push(new_s);
        }

        let logits = self.lm_head.forward(&x)?;

        Ok((logits, next_states))
    }

    /// 🌀 Boucle de Génération (Inférence Autorégressive State-Space)
    #[instrument(skip_all)]
    pub fn generate(
        &self,
        prompt_tokens: &[u32],
        max_tokens: usize,
        device: &Device,
    ) -> Result<Vec<u32>> {
        info!("🔮 Démarrage de la boucle Chimera Autorégressive (SSM Cache O(1))");
        let mut context = prompt_tokens.to_vec();

        // Le cache vec recyclé !
        let mut ssm_states: Option<Vec<Vec<f32>>> = None;

        if !context.is_empty() {
            let context_tensor = Tensor::new(context.as_slice(), device)?;
            let (logits, states) = self.forward(&context_tensor, None)?;
            ssm_states = Some(states);

            let seq_len = logits.dim(0)?;
            let next_token_logits = logits.i((seq_len - 1, ..))?;
            let next_token = next_token_logits.argmax(D::Minus1)?.to_scalar::<u32>()?;
            context.push(next_token);
        } else {
            context.push(0);
        }

        for step in 0..max_tokens {
            let last_token = *context.last().unwrap();
            let token_tensor = Tensor::new(&[last_token], device)?;

            // On prend ownership du tuple ssm_states via .take() pour le recycler (Zero-Memory leak)
            let passed_states = ssm_states.take();
            let (logits, states) = self.forward(&token_tensor, passed_states)?;
            ssm_states = Some(states);

            let next_token = logits.i((0, ..))?.argmax(D::Minus1)?.to_scalar::<u32>()?;
            context.push(next_token);
            tracing::debug!("Token n°{} : ID [{}]", step + 1, next_token);
        }

        let generated_slice = context[prompt_tokens.len()..].to_vec();
        info!(
            "✅ Inférence Chimera terminée ({} tokens générés avec cache SSM)",
            generated_slice.len()
        );
        Ok(generated_slice)
    }
}
