use crate::error::Result;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

pub const DEFAULT_MODEL: &str = "minilm";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelType {
    MiniLM,
    Nomic,
}

impl std::str::FromStr for ModelType {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "minilm" | "all-minilm-l6-v2" => Ok(ModelType::MiniLM),
            "nomic" | "nomic-embed-text-v1.5" => Ok(ModelType::Nomic),
            _ => Ok(ModelType::MiniLM),
        }
    }
}

impl ModelType {
    pub fn parse(s: &str) -> Self {
        s.parse().unwrap_or(ModelType::MiniLM)
    }

    pub fn dimension(&self) -> usize {
        match self {
            ModelType::MiniLM => 384,
            ModelType::Nomic => 768,
        }
    }

    pub fn document_prefix(&self) -> &'static str {
        match self {
            ModelType::MiniLM => "",
            ModelType::Nomic => "search_document: ",
        }
    }

    pub fn query_prefix(&self) -> &'static str {
        match self {
            ModelType::MiniLM => "",
            ModelType::Nomic => "search_query: ",
        }
    }
}

static ONNX_AVAILABLE: OnceLock<bool> = OnceLock::new();

fn check_onnx_available() -> bool {
    *ONNX_AVAILABLE.get_or_init(|| {
        #[cfg(feature = "onnx")]
        {
            true
        }
        #[cfg(not(feature = "onnx"))]
        {
            false
        }
    })
}

#[cfg(feature = "onnx")]
mod onnx_backend {
    use super::*;
    use crate::error::CodeSearchError;
    use hf_hub::api::sync::Api;
    use ndarray::Array2;
    use ort::session::{builder::GraphOptimizationLevel, Session};
    use ort::value::Tensor;
    use std::sync::{Arc, RwLock};
    use tokenizers::Tokenizer;

    pub struct LoadedModel {
        session: Session,
        tokenizer: Tokenizer,
    }

    impl LoadedModel {
        pub fn new(model_type: ModelType) -> Result<Self> {
            let api = Api::new().map_err(|e| {
                CodeSearchError::EmbeddingModelLoad(format!(
                    "Failed to initialize HuggingFace API: {}",
                    e
                ))
            })?;

            let repo = api.model(model_type.repo_id().to_string());

            let model_path = repo.get("onnx/model.onnx").map_err(|e| {
                CodeSearchError::EmbeddingModelLoad(format!("Failed to download model: {}", e))
            })?;

            let tokenizer_path = repo.get("tokenizer.json").map_err(|e| {
                CodeSearchError::EmbeddingModelLoad(format!("Failed to download tokenizer: {}", e))
            })?;

            let session = Session::builder()
                .map_err(|e| {
                    CodeSearchError::EmbeddingModelLoad(format!(
                        "Failed to create session builder: {}",
                        e
                    ))
                })?
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(|e| {
                    CodeSearchError::EmbeddingModelLoad(format!(
                        "Failed to set optimization level: {}",
                        e
                    ))
                })?
                .commit_from_file(&model_path)
                .map_err(|e| {
                    CodeSearchError::EmbeddingModelLoad(format!(
                        "Failed to load model from file: {}",
                        e
                    ))
                })?;

            let tokenizer = Tokenizer::from_file(&tokenizer_path)
                .map_err(|e| {
                    CodeSearchError::EmbeddingModelLoad(format!("Failed to load tokenizer: {}", e))
                })?
                .into();

            Ok(Self { session, tokenizer })
        }

        pub fn encode(&mut self, text: &str) -> Result<Vec<f32>> {
            let encoding = self.tokenizer.encode(text, true).map_err(|e| {
                CodeSearchError::EmbeddingInference(format!("Tokenization failed: {}", e))
            })?;

            let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
            let attention_mask: Vec<i64> = encoding
                .get_attention_mask()
                .iter()
                .map(|&m| m as i64)
                .collect();
            let token_type_ids: Vec<i64> = encoding
                .get_type_ids()
                .iter()
                .map(|id| *id as i64)
                .collect();

            let seq_len = encoding.len();

            let input_ids_array = Array2::from_shape_vec((1, seq_len), input_ids).map_err(|e| {
                CodeSearchError::EmbeddingInference(format!("Input shape error: {}", e))
            })?;
            let attention_mask_array = Array2::from_shape_vec((1, seq_len), attention_mask.clone())
                .map_err(|e| {
                    CodeSearchError::EmbeddingInference(format!(
                        "Attention mask shape error: {}",
                        e
                    ))
                })?;
            let token_type_ids_array = Array2::from_shape_vec((1, seq_len), token_type_ids)
                .map_err(|e| {
                    CodeSearchError::EmbeddingInference(format!("Token type shape error: {}", e))
                })?;

            let input_ids_tensor = Tensor::<i64>::from_array(input_ids_array).map_err(|e| {
                CodeSearchError::EmbeddingInference(format!(
                    "Failed to create input_ids tensor: {}",
                    e
                ))
            })?;
            let attention_mask_tensor =
                Tensor::<i64>::from_array(attention_mask_array).map_err(|e| {
                    CodeSearchError::EmbeddingInference(format!(
                        "Failed to create attention_mask tensor: {}",
                        e
                    ))
                })?;
            let token_type_ids_tensor =
                Tensor::<i64>::from_array(token_type_ids_array).map_err(|e| {
                    CodeSearchError::EmbeddingInference(format!(
                        "Failed to create token_type_ids tensor: {}",
                        e
                    ))
                })?;

            let outputs = self
                .session
                .run(ort::inputs![
                    "input_ids" => input_ids_tensor,
                    "attention_mask" => attention_mask_tensor,
                    "token_type_ids" => token_type_ids_tensor,
                ])
                .map_err(|e| {
                    CodeSearchError::EmbeddingInference(format!("Inference failed: {}", e))
                })?;

            let last_hidden_state = outputs["last_hidden_state"]
                .try_extract_tensor::<f32>()
                .map_err(|e| {
                    CodeSearchError::EmbeddingInference(format!("Failed to extract tensor: {}", e))
                })?;

            let (shape, data) = last_hidden_state;
            let seq_len_out = shape[1] as usize;
            let hidden_size = shape[2] as usize;

            let embedding = mean_pool(data, &attention_mask, seq_len_out, hidden_size);
            let normalized = l2_normalize(&embedding);

            Ok(normalized)
        }

        pub fn encode_batch(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            texts.iter().map(|text| self.encode(text)).collect()
        }
    }

    fn mean_pool(
        hidden_state: &[f32],
        attention_mask: &[i64],
        seq_len: usize,
        hidden_size: usize,
    ) -> Vec<f32> {
        let mut sum = vec![0.0_f32; hidden_size];
        let mut count = 0.0_f32;

        for i in 0..seq_len {
            if i < attention_mask.len() && attention_mask[i] == 1 {
                for j in 0..hidden_size {
                    sum[j] += hidden_state[i * hidden_size + j];
                }
                count += 1.0;
            }
        }

        if count > 0.0 {
            sum.iter().map(|v| v / count).collect()
        } else {
            sum
        }
    }

    pub struct GlobalEmbedder {
        model: Arc<RwLock<Option<LoadedModel>>>,
        model_type: ModelType,
    }

    impl GlobalEmbedder {
        pub fn new(model_type: ModelType) -> Self {
            Self {
                model: Arc::new(RwLock::new(None)),
                model_type,
            }
        }

        pub fn ensure_loaded(&self) -> Result<()> {
            {
                let read_guard = self.model.read().map_err(|e| {
                    CodeSearchError::EmbeddingModelLoad(format!("Lock error: {}", e))
                })?;
                if read_guard.is_some() {
                    return Ok(());
                }
            }

            let mut write_guard = self
                .model
                .write()
                .map_err(|e| CodeSearchError::EmbeddingModelLoad(format!("Lock error: {}", e)))?;

            if write_guard.is_none() {
                let model = LoadedModel::new(self.model_type)?;
                *write_guard = Some(model);
            }

            Ok(())
        }

        pub fn get_embedding_with_prefix(&self, text: &str, prefix: &str) -> Result<Vec<f32>> {
            self.ensure_loaded()?;
            let mut guard = self
                .model
                .write()
                .map_err(|e| CodeSearchError::EmbeddingInference(format!("Lock error: {}", e)))?;
            let model = guard.as_mut().ok_or_else(|| {
                CodeSearchError::EmbeddingInference("Model not loaded".to_string())
            })?;
            let prefixed_text = format!("{}{}", prefix, text);
            model.encode(&prefixed_text)
        }

        pub fn get_embeddings_batch(
            &self,
            texts: &[String],
            batch_size: usize,
            is_query: bool,
        ) -> Result<Vec<Vec<f32>>> {
            self.ensure_loaded()?;
            let mut guard = self
                .model
                .write()
                .map_err(|e| CodeSearchError::EmbeddingInference(format!("Lock error: {}", e)))?;
            let model = guard.as_mut().ok_or_else(|| {
                CodeSearchError::EmbeddingInference("Model not loaded".to_string())
            })?;

            let prefix = if is_query {
                self.model_type.query_prefix()
            } else {
                self.model_type.document_prefix()
            };

            let mut all_embeddings = Vec::with_capacity(texts.len());

            for chunk in texts.chunks(batch_size) {
                let prefixed_texts: Vec<String> = chunk
                    .iter()
                    .map(|text| format!("{}{}", prefix, text))
                    .collect();

                let embeddings = model.encode_batch(&prefixed_texts)?;
                all_embeddings.extend(embeddings);
            }

            Ok(all_embeddings)
        }

        pub fn check_available(&self) -> bool {
            self.ensure_loaded().is_ok()
        }

        pub fn is_loaded(&self) -> bool {
            let guard = self.model.read();
            match guard {
                Ok(g) => g.is_some(),
                Err(_) => false,
            }
        }
    }

    impl ModelType {
        pub fn repo_id(&self) -> &'static str {
            match self {
                ModelType::MiniLM => "sentence-transformers/all-MiniLM-L6-v2",
                ModelType::Nomic => "nomic-ai/nomic-embed-text-v1.5",
            }
        }
    }
}

#[cfg(not(feature = "onnx"))]
mod fallback_backend {
    use super::*;

    pub struct GlobalEmbedder {
        model_type: ModelType,
    }

    impl GlobalEmbedder {
        pub fn new(model_type: ModelType) -> Self {
            Self { model_type }
        }

        pub fn ensure_loaded(&self) -> Result<()> {
            Ok(())
        }

        pub fn get_embedding_with_prefix(&self, text: &str, prefix: &str) -> Result<Vec<f32>> {
            let prefixed_text = format!("{}{}", prefix, text);
            Ok(hash_to_embedding(
                &prefixed_text,
                self.model_type.dimension(),
            ))
        }

        pub fn get_embeddings_batch(
            &self,
            texts: &[String],
            _batch_size: usize,
            is_query: bool,
        ) -> Result<Vec<Vec<f32>>> {
            let prefix = if is_query {
                self.model_type.query_prefix()
            } else {
                self.model_type.document_prefix()
            };

            Ok(texts
                .iter()
                .map(|text| {
                    let prefixed_text = format!("{}{}", prefix, text);
                    hash_to_embedding(&prefixed_text, self.model_type.dimension())
                })
                .collect())
        }

        pub fn check_available(&self) -> bool {
            true
        }

        pub fn is_loaded(&self) -> bool {
            true
        }
    }
}

fn hash_to_embedding(text: &str, dim: usize) -> Vec<f32> {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let hash = hasher.finalize();

    let mut embedding = Vec::with_capacity(dim);

    for i in 0..dim {
        let offset = (i * 2) % 28;
        let value = i32::from_le_bytes([
            hash[offset],
            hash[offset + 1],
            hash[(offset + 2) % 32],
            hash[(offset + 3) % 32],
        ]);
        embedding.push(value as f32 / 1e9);
    }

    l2_normalize(&embedding)
}

fn l2_normalize(embedding: &[f32]) -> Vec<f32> {
    let norm: f32 = embedding.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        embedding.iter().map(|v| v / norm).collect()
    } else {
        embedding.to_vec()
    }
}

#[cfg(feature = "onnx")]
use onnx_backend::GlobalEmbedder;

#[cfg(not(feature = "onnx"))]
use fallback_backend::GlobalEmbedder;

static MINILM_EMBEDDER: OnceLock<GlobalEmbedder> = OnceLock::new();
static NOMIC_EMBEDDER: OnceLock<GlobalEmbedder> = OnceLock::new();

fn get_embedder(model_type: ModelType) -> &'static GlobalEmbedder {
    match model_type {
        ModelType::MiniLM => MINILM_EMBEDDER.get_or_init(|| GlobalEmbedder::new(ModelType::MiniLM)),
        ModelType::Nomic => NOMIC_EMBEDDER.get_or_init(|| GlobalEmbedder::new(ModelType::Nomic)),
    }
}

#[derive(Clone)]
pub struct EmbeddingModel {
    model_type: ModelType,
}

impl EmbeddingModel {
    pub fn new(model_name: Option<&str>) -> Result<Self> {
        let model_type = ModelType::parse(model_name.unwrap_or(DEFAULT_MODEL));
        let embedder = get_embedder(model_type);
        embedder.ensure_loaded()?;
        Ok(Self { model_type })
    }

    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let embedder = get_embedder(self.model_type);
        embedder.get_embedding_with_prefix(text, self.model_type.document_prefix())
    }

    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let embedder = get_embedder(self.model_type);
        let texts: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        embedder.get_embeddings_batch(&texts, texts.len(), false)
    }

    pub fn embedding_dimension(&self) -> usize {
        self.model_type.dimension()
    }
}

pub fn get_embedding(text: &str) -> Vec<f32> {
    get_embedding_with_model(text, DEFAULT_MODEL)
}

pub fn get_embedding_with_model(text: &str, model: &str) -> Vec<f32> {
    let model_type = ModelType::parse(model);
    let embedder = get_embedder(model_type);
    let prefix = model_type.document_prefix();
    embedder
        .get_embedding_with_prefix(text, prefix)
        .unwrap_or_else(|_| vec![0.0; model_type.dimension()])
}

pub fn get_query_embedding(text: &str) -> Vec<f32> {
    get_query_embedding_with_model(text, DEFAULT_MODEL)
}

pub fn get_query_embedding_with_model(text: &str, model: &str) -> Vec<f32> {
    let model_type = ModelType::parse(model);
    let embedder = get_embedder(model_type);
    let prefix = model_type.query_prefix();
    embedder
        .get_embedding_with_prefix(text, prefix)
        .unwrap_or_else(|_| vec![0.0; model_type.dimension()])
}

pub fn get_embeddings_batch(texts: &[String], batch_size: usize, is_query: bool) -> Vec<Vec<f32>> {
    get_embeddings_batch_with_model(texts, batch_size, is_query, DEFAULT_MODEL)
}

pub fn get_embeddings_batch_with_model(
    texts: &[String],
    batch_size: usize,
    is_query: bool,
    model: &str,
) -> Vec<Vec<f32>> {
    let model_type = ModelType::parse(model);
    let embedder = get_embedder(model_type);
    embedder
        .get_embeddings_batch(texts, batch_size, is_query)
        .unwrap_or_else(|_| {
            texts
                .iter()
                .map(|_| vec![0.0; model_type.dimension()])
                .collect()
        })
}

pub fn check_available() -> bool {
    check_available_with_model(DEFAULT_MODEL)
}

pub fn check_available_with_model(model: &str) -> bool {
    let model_type = ModelType::parse(model);
    let embedder = get_embedder(model_type);
    embedder.check_available()
}

pub fn ensure_model_available() -> Result<()> {
    ensure_model_available_with_model(DEFAULT_MODEL)
}

pub fn ensure_model_available_with_model(model: &str) -> Result<()> {
    let model_type = ModelType::parse(model);
    let embedder = get_embedder(model_type);
    embedder.ensure_loaded()
}

pub fn get_model_dimension(model: &str) -> usize {
    ModelType::parse(model).dimension()
}

pub fn is_model_loaded(model: &str) -> bool {
    let model_type = ModelType::parse(model);
    let embedder = get_embedder(model_type);
    embedder.is_loaded()
}

pub fn zero_embedding() -> Vec<f32> {
    vec![0.0_f32; ModelType::MiniLM.dimension()]
}

pub fn zero_embedding_with_model(model: &str) -> Vec<f32> {
    vec![0.0_f32; ModelType::parse(model).dimension()]
}

pub fn is_onnx_available() -> bool {
    check_onnx_available()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_type_from_str() {
        assert_eq!(ModelType::parse("minilm"), ModelType::MiniLM);
        assert_eq!(ModelType::parse("MiniLM"), ModelType::MiniLM);
        assert_eq!(ModelType::parse("all-minilm-l6-v2"), ModelType::MiniLM);
        assert_eq!(ModelType::parse("nomic"), ModelType::Nomic);
        assert_eq!(ModelType::parse("Nomic"), ModelType::Nomic);
        assert_eq!(ModelType::parse("unknown"), ModelType::MiniLM);
    }

    #[test]
    fn test_model_dimensions() {
        assert_eq!(ModelType::MiniLM.dimension(), 384);
        assert_eq!(ModelType::Nomic.dimension(), 768);
    }

    #[test]
    fn test_model_prefixes() {
        assert_eq!(ModelType::MiniLM.document_prefix(), "");
        assert_eq!(ModelType::MiniLM.query_prefix(), "");
        assert_eq!(ModelType::Nomic.document_prefix(), "search_document: ");
        assert_eq!(ModelType::Nomic.query_prefix(), "search_query: ");
    }

    #[test]
    fn test_l2_normalize() {
        let embedding = vec![3.0, 4.0];
        let normalized = l2_normalize(&embedding);
        assert!((normalized[0] - 0.6).abs() < 1e-6);
        assert!((normalized[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_l2_normalize_zero_vector() {
        let embedding = vec![0.0, 0.0, 0.0];
        let normalized = l2_normalize(&embedding);
        assert_eq!(normalized, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_zero_embedding() {
        let embedding = zero_embedding();
        assert_eq!(embedding.len(), 384);
        assert!(embedding.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn test_get_model_dimension() {
        assert_eq!(get_model_dimension("minilm"), 384);
        assert_eq!(get_model_dimension("nomic"), 768);
    }

    #[test]
    fn test_hash_to_embedding() {
        let emb1 = hash_to_embedding("hello world", 384);
        let emb2 = hash_to_embedding("hello world", 384);
        let emb3 = hash_to_embedding("different text", 384);

        assert_eq!(emb1.len(), 384);
        assert_eq!(emb1, emb2);

        let norm: f32 = emb1.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);

        let diff_count = emb1
            .iter()
            .zip(emb3.iter())
            .filter(|(a, b)| (**a - **b).abs() > 1e-6)
            .count();
        assert!(
            diff_count > 0,
            "Different inputs should produce different embeddings"
        );
    }

    #[test]
    fn test_fallback_embedding() {
        let emb = get_embedding("test query");
        assert_eq!(emb.len(), 384);

        let query_emb = get_query_embedding("test query");
        assert_eq!(query_emb.len(), 384);
    }
}
