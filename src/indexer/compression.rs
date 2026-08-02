use std::cell::RefCell;
use std::io::Read;

use anyhow::{Context, Result};

pub(super) const ZSTD_MAGIC: &[u8] = &[0x28, 0xB5, 0x2F, 0xFD];
const MIN_COMPRESSED_TEXT_BYTES: usize = 512;
const MAX_DECOMPRESSED_TEXT_BYTES: u64 = 32 * 1024 * 1024;

thread_local! {
    static TEXT_COMPRESSOR: RefCell<Option<zstd::bulk::Compressor<'static>>> =
        RefCell::new(zstd::bulk::Compressor::new(1).ok());
}

pub(super) fn compress_text(text: &str) -> Vec<u8> {
    let raw = text.as_bytes();
    if raw.len() < MIN_COMPRESSED_TEXT_BYTES {
        return raw.to_vec();
    }

    TEXT_COMPRESSOR
        .with_borrow_mut(|compressor| {
            compressor
                .as_mut()
                .and_then(|value| value.compress(raw).ok())
        })
        .filter(|compressed| compressed.len() < raw.len())
        .unwrap_or_else(|| raw.to_vec())
}

/// Decode text stored in the chunk database.
///
/// Plain text remains lossy for compatibility with indexes created from
/// non-UTF-8 source. A value carrying the zstd frame marker is never treated as
/// plain text: corrupt, truncated, oversized, or invalid UTF-8 frames fail.
pub fn try_decompress_text(raw: Vec<u8>) -> Result<String> {
    if !raw.starts_with(ZSTD_MAGIC) {
        return Ok(String::from_utf8(raw)
            .unwrap_or_else(|error| String::from_utf8_lossy(&error.into_bytes()).into_owned()));
    }

    let decoder =
        zstd::stream::read::Decoder::new(&raw[..]).context("invalid zstd frame in stored chunk")?;
    let mut decoded = Vec::new();
    decoder
        .take(MAX_DECOMPRESSED_TEXT_BYTES + 1)
        .read_to_end(&mut decoded)
        .context("failed to decompress stored chunk")?;
    anyhow::ensure!(
        decoded.len() as u64 <= MAX_DECOMPRESSED_TEXT_BYTES,
        "decompressed stored chunk exceeds {MAX_DECOMPRESSED_TEXT_BYTES} bytes"
    );
    String::from_utf8(decoded).context("decompressed stored chunk is not UTF-8")
}

/// Compatibility wrapper for callers that predate fallible chunk decoding.
///
/// New persisted-data boundaries should use [`try_decompress_text`]. Corrupt
/// compressed data is rendered as an explicit diagnostic instead of being
/// mistaken for source text.
pub fn decompress_text(raw: Vec<u8>) -> String {
    try_decompress_text(raw)
        .unwrap_or_else(|error| format!("[ivygrep: corrupt stored chunk: {error:#}]"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_chunks_stay_plain() {
        let original = "pub fn hello() -> &str { \"world\" }\n";
        let compressed = compress_text(original);
        assert!(!compressed.starts_with(ZSTD_MAGIC));
        assert_eq!(try_decompress_text(compressed).unwrap(), original);
    }

    #[test]
    fn large_chunks_roundtrip_with_zstd() {
        let original = "pub fn hello() -> &str { \"world\" }\n".repeat(64);
        let compressed = compress_text(&original);
        assert!(compressed.starts_with(ZSTD_MAGIC));
        assert!(compressed.len() < original.len());
        assert_eq!(try_decompress_text(compressed).unwrap(), original);
    }

    #[test]
    fn plain_non_utf8_text_keeps_legacy_lossy_behavior() {
        assert_eq!(try_decompress_text(vec![b'a', 0xff]).unwrap(), "a�");
    }

    #[test]
    fn corrupted_zstd_is_reported_instead_of_returned_as_gibberish() {
        let mut corrupted = zstd::encode_all(&b"valid stored text"[..], 1).unwrap();
        corrupted.truncate(corrupted.len() - 3);

        let error = try_decompress_text(corrupted.clone()).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("failed to decompress stored chunk")
        );
        assert!(decompress_text(corrupted).starts_with("[ivygrep: corrupt stored chunk:"));
    }
}
