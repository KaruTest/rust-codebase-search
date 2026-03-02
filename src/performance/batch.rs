// Batch Embedding Optimization Module
// Provides efficient batch processing with progress reporting and GPU acceleration

use crate::embedding::get_embeddings_batch_with_model;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// Configuration for batch embedding
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Default batch size for embedding generation
    pub batch_size: usize,
    /// Enable GPU acceleration if available
    pub use_gpu: bool,
    /// Show progress bar
    pub show_progress: bool,
    /// Number of worker threads
    pub num_workers: usize,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            batch_size: 32,
            use_gpu: true,
            show_progress: true,
            num_workers: 4,
        }
    }
}

/// Batch embedding result with statistics
#[derive(Debug, Clone)]
pub struct BatchResult {
    pub embeddings: Vec<Vec<f32>>,
    pub total_items: usize,
    pub batches: usize,
    pub duration_ms: u64,
    pub items_per_second: f64,
}

/// Progress reporter for batch processing
pub struct BatchProgress {
    pb: Option<ProgressBar>,
    total: usize,
    completed: AtomicUsize,
    cancelled: AtomicBool,
}

impl BatchProgress {
    pub fn new(total: usize, show_progress: bool) -> Self {
        let pb = if show_progress && total > 0 {
            let pb = ProgressBar::new(total as u64);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
                    .unwrap()
                    .progress_chars("#>-"),
            );
            Some(pb)
        } else {
            None
        };

        Self {
            pb,
            total,
            completed: AtomicUsize::new(0),
            cancelled: AtomicBool::new(false),
        }
    }

    pub fn increment(&self, amount: usize) {
        let new_count = self.completed.fetch_add(amount, Ordering::Relaxed) + amount;
        if let Some(ref pb) = self.pb {
            pb.set_position(new_count as u64);
            let pct = if self.total > 0 {
                (new_count * 100) / self.total
            } else {
                0
            };
            pb.set_message(format!("{}%", pct));
        }
    }

    pub fn finish(&self) {
        if let Some(ref pb) = self.pb {
            pb.finish();
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        if let Some(ref pb) = self.pb {
            pb.abandon();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

/// Process embeddings in batches with progress reporting
pub fn process_embeddings_batch(
    texts: &[String],
    model: &str,
    config: &BatchConfig,
) -> BatchResult {
    let start = std::time::Instant::now();
    let total_items = texts.len();

    if texts.is_empty() {
        return BatchResult {
            embeddings: Vec::new(),
            total_items: 0,
            batches: 0,
            duration_ms: 0,
            items_per_second: 0.0,
        };
    }

    let progress = Arc::new(BatchProgress::new(total_items, config.show_progress));

    let batch_size = config.batch_size;
    let num_batches = (total_items + batch_size - 1) / batch_size;

    let mut all_embeddings = Vec::with_capacity(total_items);

    // Process in batches using rayon for parallelization
    let chunks: Vec<Vec<String>> = texts
        .chunks(batch_size)
        .map(|c| c.to_vec())
        .collect();

    // Process batches in parallel
    let results: Vec<Vec<Vec<f32>>> = chunks
        .par_iter()
        .map(|chunk| {
            if progress.is_cancelled() {
                return vec![Vec::new(); chunk.len()];
            }

            let embeddings = get_embeddings_batch_with_model(chunk, chunk.len(), false, model);
            progress.increment(chunk.len());
            embeddings
        })
        .collect();

    // Flatten results
    for batch_embeddings in results {
        all_embeddings.extend(batch_embeddings);
    }

    progress.finish();

    let duration = start.elapsed();
    let items_per_second = if duration.as_millis() > 0 {
        (total_items as f64) / (duration.as_millis() as f64 / 1000.0)
    } else {
        0.0
    };

    BatchResult {
        embeddings: all_embeddings,
        total_items,
        batches: num_batches,
        duration_ms: duration.as_millis() as u64,
        items_per_second,
    }
}

/// Process embeddings in batches with callback for progress
pub fn process_embeddings_with_callback<F>(
    texts: &[String],
    model: &str,
    batch_size: usize,
    show_progress: bool,
    callback: F,
) -> BatchResult
where
    F: Fn(usize, usize) + Send + Sync,
{
    let start = std::time::Instant::now();
    let total_items = texts.len();

    if texts.is_empty() {
        return BatchResult {
            embeddings: Vec::new(),
            total_items: 0,
            batches: 0,
            duration_ms: 0,
            items_per_second: 0.0,
        };
    }

    let progress = Arc::new(BatchProgress::new(total_items, show_progress));
    let callback = Arc::new(callback);

    let batches: Vec<&[String]> = texts.chunks(batch_size).collect();
    let num_batches = batches.len();

    let mut all_embeddings = Vec::with_capacity(total_items);

    for (batch_idx, batch) in batches.iter().enumerate() {
        let embeddings = get_embeddings_batch_with_model(*batch, batch.len(), false, model);
        all_embeddings.extend(embeddings);

        let completed = ((batch_idx + 1) * batch.len()).min(total_items);
        progress.increment(batch.len());
        callback(batch_idx + 1, num_batches);
    }

    progress.finish();

    let duration = start.elapsed();
    let items_per_second = if duration.as_millis() > 0 {
        (total_items as f64) / (duration.as_millis() as f64 / 1000.0)
    } else {
        0.0
    };

    BatchResult {
        embeddings: all_embeddings,
        total_items,
        batches: num_batches,
        duration_ms: duration.as_millis() as u64,
        items_per_second,
    }
}

/// Detect if GPU acceleration is available
pub fn detect_gpu_acceleration() -> GpuInfo {
    #[cfg(feature = "onnx")]
    {
        // Check for CUDA
        if let Ok(available) = std::env::var("ORT_CUDA") {
            if available == "1" {
                return GpuInfo {
                    available: true,
                    name: "CUDA".to_string(),
                    provider: GpuProvider::Cuda,
                };
            }
        }

        // Check for CoreML (Apple Silicon)
        if let Ok(available) = std::env::var("ORT_COREML") {
            if available == "1" {
                return GpuInfo {
                    available: true,
                    name: "CoreML".to_string(),
                    provider: GpuProvider::CoreML,
                };
            }
        }
    }

    GpuInfo {
        available: false,
        name: "CPU only".to_string(),
        provider: GpuProvider::None,
    }
}

/// GPU information
#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub available: bool,
    pub name: String,
    pub provider: GpuProvider,
}

/// GPU provider type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuProvider {
    None,
    Cuda,
    CoreML,
}

/// Optimal batch size based on hardware
pub fn get_optimal_batch_size() -> usize {
    let gpu_info = detect_gpu_acceleration();

    // GPU can handle larger batches
    if gpu_info.available {
        match gpu_info.provider {
            GpuProvider::Cuda => 128,
            GpuProvider::CoreML => 64,
            GpuProvider::None => 32,
        }
    } else {
        // CPU batch size based on cores
        let cores = num_cpus::get();
        match cores {
            0..=2 => 16,
            3..=4 => 32,
            5..=8 => 48,
            _ => 64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_processing_empty() {
        let config = BatchConfig::default();
        let result = process_embeddings_batch(&[], "minilm", &config);

        assert_eq!(result.embeddings.len(), 0);
        assert_eq!(result.total_items, 0);
    }

    #[test]
    fn test_batch_processing() {
        let config = BatchConfig {
            batch_size: 10,
            show_progress: false,
            ..Default::default()
        };

        let texts: Vec<String> = (0..20).map(|i| format!("test text {}", i)).collect();
        let result = process_embeddings_batch(&texts, "minilm", &config);

        assert_eq!(result.embeddings.len(), 20);
        assert_eq!(result.total_items, 20);
        assert!(result.duration_ms > 0);
    }

    #[test]
    fn test_gpu_detection() {
        let info = detect_gpu_acceleration();
        // Should not panic, just return info
        assert!(info.name.len() > 0);
    }

    #[test]
    fn test_optimal_batch_size() {
        let size = get_optimal_batch_size();
        assert!(size > 0);
    }
}
