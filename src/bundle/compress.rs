use flate2::Compression;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};
use std::io::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionAlgo {
    #[serde(rename = "gzip")]
    Gzip,
    #[serde(rename = "brotli")]
    Brotli,
    #[serde(rename = "zstd")]
    Zstd,
    #[serde(rename = "raw")]
    Raw,
    #[serde(rename = "all")]
    All,
}

impl std::str::FromStr for CompressionAlgo {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "gzip" | "gz" => Ok(CompressionAlgo::Gzip),
            "brotli" | "br" => Ok(CompressionAlgo::Brotli),
            "zstd" | "zst" => Ok(CompressionAlgo::Zstd),
            "raw" | "none" => Ok(CompressionAlgo::Raw),
            "all" => Ok(CompressionAlgo::All),
            _ => Err(format!(
                "Unknown compression algorithm: {s}. Supported: gzip, brotli, zstd, raw, all"
            )),
        }
    }
}

impl std::fmt::Display for CompressionAlgo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompressionAlgo::Gzip => write!(f, "gzip"),
            CompressionAlgo::Brotli => write!(f, "brotli"),
            CompressionAlgo::Zstd => write!(f, "zstd"),
            CompressionAlgo::Raw => write!(f, "raw"),
            CompressionAlgo::All => write!(f, "all"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompressionMetrics {
    pub raw_bytes: usize,
    pub gzip_bytes: usize,
    pub brotli_bytes: usize,
    pub zstd_bytes: usize,
    pub gzip_ratio: f64,
    pub brotli_ratio: f64,
    pub zstd_ratio: f64,
}

impl CompressionMetrics {
    pub fn calculate(data: &[u8]) -> Self {
        let raw_bytes = data.len();
        let gzip_bytes = compress_gzip(data).map(|v| v.len()).unwrap_or(raw_bytes);
        let brotli_bytes = compress_brotli(data).map(|v| v.len()).unwrap_or(raw_bytes);
        let zstd_bytes = compress_zstd(data).map(|v| v.len()).unwrap_or(raw_bytes);

        let gzip_ratio = if raw_bytes > 0 {
            gzip_bytes as f64 / raw_bytes as f64
        } else {
            0.0
        };
        let brotli_ratio = if raw_bytes > 0 {
            brotli_bytes as f64 / raw_bytes as f64
        } else {
            0.0
        };
        let zstd_ratio = if raw_bytes > 0 {
            zstd_bytes as f64 / raw_bytes as f64
        } else {
            0.0
        };

        Self {
            raw_bytes,
            gzip_bytes,
            brotli_bytes,
            zstd_bytes,
            gzip_ratio,
            brotli_ratio,
            zstd_ratio,
        }
    }

    pub fn size_for(&self, algo: CompressionAlgo) -> usize {
        match algo {
            CompressionAlgo::Gzip => self.gzip_bytes,
            CompressionAlgo::Brotli => self.brotli_bytes,
            CompressionAlgo::Zstd => self.zstd_bytes,
            CompressionAlgo::Raw => self.raw_bytes,
            CompressionAlgo::All => self.gzip_bytes,
        }
    }
}

pub fn compress_gzip(data: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data)?;
    encoder.finish()
}

pub fn compress_brotli(data: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut writer = brotli::CompressorWriter::new(&mut output, 4096, 11, 22);
    writer.write_all(data)?;
    drop(writer);
    Ok(output)
}

pub fn compress_zstd(data: &[u8]) -> std::io::Result<Vec<u8>> {
    zstd::encode_all(data, 19)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_engines() {
        let sample = b"Hello Cloudflare Workers! This is a test bundle script with repetitive content. \
                       Hello Cloudflare Workers! This is a test bundle script with repetitive content. \
                       Hello Cloudflare Workers! This is a test bundle script with repetitive content.";

        let gz = compress_gzip(sample).expect("gzip failed");
        let br = compress_brotli(sample).expect("brotli failed");
        let zst = compress_zstd(sample).expect("zstd failed");

        assert!(gz.len() < sample.len());
        assert!(br.len() < sample.len());
        assert!(zst.len() < sample.len());

        let metrics = CompressionMetrics::calculate(sample);
        assert_eq!(metrics.raw_bytes, sample.len());
        assert_eq!(metrics.gzip_bytes, gz.len());
        assert_eq!(metrics.brotli_bytes, br.len());
        assert_eq!(metrics.zstd_bytes, zst.len());
        assert!(metrics.gzip_ratio < 1.0);
    }
}
