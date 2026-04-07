use crate::hadamard::HadamardLayer;
use crate::moe::{Expert, SparseMoe};
use crate::ssm::SsmBlock;
use candle_core::{Device, IndexOp, Result, Tensor, D};
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

    /// Profil Giga-Model (Production 3B)
    pub fn b1_58_3b() -> Self {
        Self {
            hidden_size: 4096, // Typique Llama-3B like
            num_hidden_layers: 32,
            num_experts: 8,
            top_k: 2,
            vocab_size: 128256, // Llama-3 128k
        }
    }
}

/// Expert "Bouchon" / Mock en attendant la vraie BitFFN
pub struct MockExpert {
    _hidden_size: usize,
}
impl Expert for MockExpert {
    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        // Identity pass for testing topological shapes
        Ok(x.clone())
    }
}

/// 🏛️ `ChimeraTransformerBlock` (Hadamard -> SSM -> MoE)
pub struct ChimeraBlock {
    pub hadamard: HadamardLayer,
    pub ssm: SsmBlock,
    pub moe: SparseMoe,
}

impl ChimeraBlock {
    pub fn forward(&self, x: &Tensor, prev_state: Option<&Tensor>) -> Result<(Tensor, Tensor)> {
        // 1. Stabilisation Quantique
        let h1 = self.hadamard.forward(x)?;

        // 2. Continuous State Space (BitMamba)
        let (ssm_out, new_state) = self.ssm.forward(&h1, prev_state)?;

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
    /// Table de mock statique pour l'Embedding (Car pas de HuggingFace)
    embed_mock: Tensor,
    /// Table de mock pour LM Head
    lm_head_mock: Tensor,
}

impl ChimeraModel {
    /// 🚀 Active le Modèle avec des poids factices pour tester la tuyauterie de bout en bout
    #[instrument(skip_all)]
    pub fn new_mocked(config: &ChimeraConfig, device: &Device) -> Result<Self> {
        info!(
            "Instanciation de la topologie Inférence ChimeraModel (Mocked) [{}] layers, dim={}, vocab={}",
            config.num_hidden_layers, config.hidden_size, config.vocab_size
        );

        let mut layers = Vec::with_capacity(config.num_hidden_layers);

        // Initialisation aléatoire ou constantes
        for _ in 0..config.num_hidden_layers {
            let dim = config.hidden_size;

            let hadamard = HadamardLayer::new(dim);

            // SSM Projections mockées = 1 ou -1
            let a = Tensor::ones((dim, dim), candle_core::DType::F32, device)?;
            let b = Tensor::ones((dim, dim), candle_core::DType::F32, device)?;
            let c = Tensor::ones((dim, dim), candle_core::DType::F32, device)?;
            let ssm = SsmBlock::new(dim, a, b, c);

            // MoE Router (Poids de la porte: [num_experts, hidden_size])
            let gate_w = Tensor::ones((config.num_experts, dim), candle_core::DType::F32, device)?;

            let mut experts: Vec<Box<dyn Expert + Send + Sync>> = Vec::new();
            for _ in 0..config.num_experts {
                experts.push(Box::new(MockExpert { _hidden_size: dim }));
            }

            let moe = SparseMoe::new(config.top_k, gate_w, experts);

            layers.push(ChimeraBlock { hadamard, ssm, moe });
        }

        // Mock tensor [vocab_size, hidden_size]
        let embed_mock = Tensor::ones(
            (config.vocab_size, config.hidden_size),
            candle_core::DType::F32,
            device,
        )?;

        let lm_head_mock = Tensor::ones(
            (config.vocab_size, config.hidden_size),
            candle_core::DType::F32,
            device,
        )?;

        Ok(Self {
            config: config.clone(),
            layers,
            embed_mock,
            lm_head_mock,
        })
    }

    /// Propagation avant sur une séquence de tokens
    #[instrument(skip_all)]
    pub fn forward(
        &self,
        tokens: &Tensor,
        prev_states: Option<&Vec<Tensor>>,
    ) -> Result<(Tensor, Vec<Tensor>)> {
        // Pseudo embedding: On récupère la ligne de `embed_mock` correspondant au Token
        let token_ids = tokens.flatten_all()?.to_vec1::<u32>()?;
        let mut embed_vectors = Vec::new();
        for &id in &token_ids {
            // Sécurité anti-crash
            let safe_id = if (id as usize) < self.config.vocab_size {
                id as usize
            } else {
                0
            };
            let row = self.embed_mock.i((safe_id, ..))?;
            embed_vectors.push(row.unsqueeze(0)?); // shape [1, hidden_size]
        }

        let mut x = Tensor::cat(&embed_vectors, 0)?; // shape [seq_len, hidden_size]

        let mut next_states = Vec::new();

        for (i, layer) in self.layers.iter().enumerate() {
            let prev_s = prev_states.and_then(|states| states.get(i));
            let (out_x, new_s) = layer.forward(&x, prev_s)?;
            x = out_x;
            next_states.push(new_s);
        }

        // Pseudo LM Head (Dot product avec lm_head_mock)
        // x shape: [seq_len, hidden_size]
        // lm_head_mock: [vocab_size, hidden_size]
        // x * lm_head_mock.T => [seq_len, vocab_size]
        let logits = x.matmul(&self.lm_head_mock.t()?)?;

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
        info!("🔮 Démarrage de la boucle Chimera Autorégressive (SSM Cache)");
        let mut context = prompt_tokens.to_vec();

        // Le cache Mamba : L'état H_t précédent pour chaque couche.
        let mut ssm_states: Option<Vec<Tensor>> = None;

        // Pré-fill : Processer le prompt entier
        if !context.is_empty() {
            let context_tensor = Tensor::new(context.as_slice(), device)?;
            let (logits, states) = self.forward(&context_tensor, None)?;
            ssm_states = Some(states);

            // Pour être rigoureux, en greedy decoding, on prend le argmax du dernier token du prompt
            let seq_len = logits.dim(0)?;
            let next_token_logits = logits.i((seq_len - 1, ..))?;
            let next_token = next_token_logits.argmax(D::Minus1)?.to_scalar::<u32>()?;
            context.push(next_token);
        } else {
            // Par défaut, générer à partir du token 0 s'il n'y a pas de prompt
            context.push(0);
        }

        for step in 0..max_tokens {
            let last_token = *context.last().unwrap();
            let token_tensor = Tensor::new(&[last_token], device)?;

            let (logits, states) = self.forward(&token_tensor, ssm_states.as_ref())?;
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
