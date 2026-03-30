use crate::attention::BitSelfAttention;
use crate::bitlinear::BitLinear;
use crate::ffn::BitFFN;
use crate::rmsnorm::RmsNorm;
use crate::transformer::BitTransformerBlock;
use crate::weights::{InferenceWeights, TrainingWeights, WeightProvider};
use candle_core::{Device, IndexOp, Module, Result, Tensor};
use candle_nn::{embedding, Embedding, VarBuilder};
use tracing::{info, instrument};

/// ⚙️ Paramétrage du Modèle R2D2-BitNet (Topologie 1.58-bit)
#[derive(Debug, Clone)]
pub struct BitNetConfig {
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub num_hidden_layers: usize,
    pub num_attention_heads: usize,
    pub vocab_size: usize,
    pub rms_norm_eps: f64,
}

impl Default for BitNetConfig {
    fn default() -> Self {
        Self {
            hidden_size: 1024,
            intermediate_size: 2048,
            num_hidden_layers: 16,
            num_attention_heads: 16, // Dimension par tête : 1024 / 16 = 64
            vocab_size: 128256,      // Llama-3 128k (Phase 1 Benchmark Winner)
            rms_norm_eps: 1e-5,
        }
    }
}

/// 🧠 Le Modèle de Langage Complet R2D2-BitNet
pub struct BitNetModel<W: WeightProvider> {
    pub config: BitNetConfig,
    embed_tokens: Embedding,
    layers: Vec<BitTransformerBlock<W>>,
    norm: RmsNorm,
    lm_head: BitLinear<W>,
}

// ==============================================================================
// 1️⃣ LOGIQUE PARTAGÉE (Forward Pass)
impl<W> BitNetModel<W>
where
    W: WeightProvider,
    BitLinear<W>: Module,
    BitTransformerBlock<W>: Module,
{
    /// Propagation avant sur une séquence de tokens
    #[instrument(skip_all)]
    pub fn forward(&self, tokens: &Tensor) -> Result<Tensor> {
        let mut x = self.embed_tokens.forward(tokens)?;

        for layer in &self.layers {
            x = layer.forward(&x)?;
        }

        x = self.norm.forward(&x)?;

        self.lm_head.forward(&x)
    }
}

// ==============================================================================
// 2️⃣ ÉTAT D'INFÉRENCE PURE (Zéro Autograd, CPU SIMD logic-only)
// ==============================================================================
impl BitNetModel<InferenceWeights> {
    #[instrument(skip_all)]
    pub fn load_inference(vb: VarBuilder, config: &BitNetConfig) -> Result<Self> {
        info!("Instanciation de la topologie Inférence BitNetModel [{}]", config.num_hidden_layers);

        let embed_tokens = embedding(config.vocab_size, config.hidden_size, vb.pp("embed_tokens"))?;

        let mut layers = Vec::with_capacity(config.num_hidden_layers);
        for layer_idx in 0..config.num_hidden_layers {
            let layer_vb = vb.pp(format!("model.layers.{}", layer_idx));

            let att_norm_w = layer_vb.pp("input_layernorm").get(config.hidden_size, "weight")?;
            let attention_norm = RmsNorm::new(att_norm_w, config.rms_norm_eps);

            let ffn_norm_w = layer_vb.pp("post_attention_layernorm").get(config.hidden_size, "weight")?;
            let ffn_norm = RmsNorm::new(ffn_norm_w, config.rms_norm_eps);

            let head_dim = config.hidden_size / config.num_attention_heads;
            let attn_vb = layer_vb.pp("self_attn");
            
            let q_proj = BitLinear::<InferenceWeights>::load(config.hidden_size, config.hidden_size, attn_vb.pp("q_proj"))?;
            let k_proj = BitLinear::<InferenceWeights>::load(config.hidden_size, config.hidden_size, attn_vb.pp("k_proj"))?;
            let v_proj = BitLinear::<InferenceWeights>::load(config.hidden_size, config.hidden_size, attn_vb.pp("v_proj"))?;
            let o_proj = BitLinear::<InferenceWeights>::load(config.hidden_size, config.hidden_size, attn_vb.pp("o_proj"))?;

            let attention = BitSelfAttention::new(config.num_attention_heads, head_dim, q_proj, k_proj, v_proj, o_proj);

            let mlp_vb = layer_vb.pp("mlp");
            let gate_proj = BitLinear::<InferenceWeights>::load(config.hidden_size, config.intermediate_size, mlp_vb.pp("gate_proj"))?;
            let up_proj = BitLinear::<InferenceWeights>::load(config.hidden_size, config.intermediate_size, mlp_vb.pp("up_proj"))?;
            let down_proj = BitLinear::<InferenceWeights>::load(config.intermediate_size, config.hidden_size, mlp_vb.pp("down_proj"))?;

            let ffn = BitFFN::new(gate_proj, up_proj, down_proj);

            layers.push(BitTransformerBlock::new(attention_norm, attention, ffn_norm, ffn));
        }

        let norm_weight = vb.pp("model.norm").get(config.hidden_size, "weight")?;
        let norm = RmsNorm::new(norm_weight, config.rms_norm_eps);
        let lm_head = BitLinear::<InferenceWeights>::load(config.hidden_size, config.vocab_size, vb.pp("lm_head"))?;

        Ok(Self { config: config.clone(), embed_tokens, layers, norm, lm_head })
    }

    /// 🌀 Boucle de Génération de Texte (Inférence Autorégressive)
    /// Exclusivité de l'état `InferenceWeights` (Isolation par le compilateur).
    #[instrument(skip_all)]
    pub fn generate(&self, prompt_tokens: &[u32], max_tokens: usize, device: &Device) -> Result<Vec<u32>> {
        info!("🔮 Démarrage de la boucle d'inférence Autorégressive (Greedy Decoding)");
        let mut context = prompt_tokens.to_vec();

        for step in 0..max_tokens {
            let context_tensor = Tensor::new(context.as_slice(), device)?.unsqueeze(0)?;
            let logits = self.forward(&context_tensor)?;
            let seq_len = logits.dim(1)?;
            let next_token_logits = logits.i((0, seq_len - 1, ..))?;
            let next_token = next_token_logits.argmax(candle_core::D::Minus1)?;
            let next_token_scalar: u32 = next_token.to_scalar()?;
            context.push(next_token_scalar);
            tracing::debug!("Token n°{} : ID [{}]", step + 1, next_token_scalar);
        }

        let generated_slice = context[prompt_tokens.len()..].to_vec();
        info!("✅ Inférence terminée ({} tokens générés)", generated_slice.len());
        Ok(generated_slice)
    }
}

// ==============================================================================
// 3️⃣ ÉTAT D'ENTRAÎNEMENT (Autograd, Poids Latents)
// ==============================================================================
impl BitNetModel<TrainingWeights> {
    #[instrument(skip_all)]
    pub fn load_train(vb: VarBuilder, config: &BitNetConfig) -> Result<Self> {
        info!("Instanciation de la topologie Entraînement BitNetModel [{}]", config.num_hidden_layers);

        let embed_tokens = embedding(config.vocab_size, config.hidden_size, vb.pp("embed_tokens"))?;

        let mut layers = Vec::with_capacity(config.num_hidden_layers);
        for layer_idx in 0..config.num_hidden_layers {
            let layer_vb = vb.pp(format!("model.layers.{}", layer_idx));

            let att_norm_w = layer_vb.pp("input_layernorm").get(config.hidden_size, "weight")?;
            let attention_norm = RmsNorm::new(att_norm_w, config.rms_norm_eps);

            let ffn_norm_w = layer_vb.pp("post_attention_layernorm").get(config.hidden_size, "weight")?;
            let ffn_norm = RmsNorm::new(ffn_norm_w, config.rms_norm_eps);

            let head_dim = config.hidden_size / config.num_attention_heads;
            let attn_vb = layer_vb.pp("self_attn");
            
            let q_proj = BitLinear::<TrainingWeights>::load_train(config.hidden_size, config.hidden_size, attn_vb.pp("q_proj"))?;
            let k_proj = BitLinear::<TrainingWeights>::load_train(config.hidden_size, config.hidden_size, attn_vb.pp("k_proj"))?;
            let v_proj = BitLinear::<TrainingWeights>::load_train(config.hidden_size, config.hidden_size, attn_vb.pp("v_proj"))?;
            let o_proj = BitLinear::<TrainingWeights>::load_train(config.hidden_size, config.hidden_size, attn_vb.pp("o_proj"))?;

            let attention = BitSelfAttention::new(config.num_attention_heads, head_dim, q_proj, k_proj, v_proj, o_proj);

            let mlp_vb = layer_vb.pp("mlp");
            let gate_proj = BitLinear::<TrainingWeights>::load_train(config.hidden_size, config.intermediate_size, mlp_vb.pp("gate_proj"))?;
            let up_proj = BitLinear::<TrainingWeights>::load_train(config.hidden_size, config.intermediate_size, mlp_vb.pp("up_proj"))?;
            let down_proj = BitLinear::<TrainingWeights>::load_train(config.intermediate_size, config.hidden_size, mlp_vb.pp("down_proj"))?;

            let ffn = BitFFN::new(gate_proj, up_proj, down_proj);

            layers.push(BitTransformerBlock::new(attention_norm, attention, ffn_norm, ffn));
        }

        let norm_weight = vb.pp("model.norm").get(config.hidden_size, "weight")?;
        let norm = RmsNorm::new(norm_weight, config.rms_norm_eps);
        let lm_head = BitLinear::<TrainingWeights>::load_train(config.hidden_size, config.vocab_size, vb.pp("lm_head"))?;

        Ok(Self { config: config.clone(), embed_tokens, layers, norm, lm_head })
    }
}
