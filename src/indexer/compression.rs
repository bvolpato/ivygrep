use std::cell::RefCell;
use std::io::Read;

use anyhow::{Context, Result};

pub(super) const ZSTD_MAGIC: &[u8] = &[0x28, 0xB5, 0x2F, 0xFD];
const MIN_COMPRESSED_TEXT_BYTES: usize = 512;
const MAX_DECOMPRESSED_TEXT_BYTES: u64 = 32 * 1024 * 1024;

thread_local! {
    static TEXT_COMPRESSOR: RefCell<Option<zstd::bulk::Compressor<'static>>> =
        RefCell::new(zstd::bulk::Compressor::new(1).ok());
    static TEXT_DECOMPRESSOR: RefCell<Option<zstd::bulk::Decompressor<'static>>> =
        RefCell::new(zstd::bulk::Decompressor::new().ok());
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

    let decoded = match decompress_sized_frame(&raw) {
        Some(decoded) => decoded,
        None => decompress_frame_stream(&raw)?,
    };
    String::from_utf8(decoded).context("decompressed stored chunk is not UTF-8")
}

/// One-shot decode of what `compress_text` writes: one frame whose header
/// records a content size within the cap. Reuses a thread-local context rather
/// than building a stream decoder and its buffers for every chunk.
///
/// Anything else (unknown size, several frames, oversized, corrupt) returns
/// `None`, so the streaming path produces the established result or error.
fn decompress_sized_frame(raw: &[u8]) -> Option<Vec<u8>> {
    let content_size = zstd::zstd_safe::get_frame_content_size(raw).ok()??;
    if content_size > MAX_DECOMPRESSED_TEXT_BYTES
        || zstd::zstd_safe::find_frame_compressed_size(raw).ok()? != raw.len()
    {
        return None;
    }
    TEXT_DECOMPRESSOR.with_borrow_mut(|decompressor| {
        let decompressor = decompressor.as_mut()?;
        let mut decoded = Vec::with_capacity(content_size as usize);
        decompressor.decompress_to_buffer(raw, &mut decoded).ok()?;
        Some(decoded)
    })
}

fn decompress_frame_stream(raw: &[u8]) -> Result<Vec<u8>> {
    let decoder =
        zstd::stream::read::Decoder::new(raw).context("invalid zstd frame in stored chunk")?;
    let mut decoded = Vec::new();
    decoder
        .take(MAX_DECOMPRESSED_TEXT_BYTES + 1)
        .read_to_end(&mut decoded)
        .context("failed to decompress stored chunk")?;
    anyhow::ensure!(
        decoded.len() as u64 <= MAX_DECOMPRESSED_TEXT_BYTES,
        "decompressed stored chunk exceeds {MAX_DECOMPRESSED_TEXT_BYTES} bytes"
    );
    Ok(decoded)
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

    #[test]
    fn sized_frame_with_wrong_content_size_is_reported() {
        let original = "pub fn hello() -> &str { \"world\" }\n".repeat(64);
        let mut corrupted = compress_text(&original);
        let declared = zstd::zstd_safe::get_frame_content_size(&corrupted).unwrap();
        assert_eq!(declared, Some(original.len() as u64));

        // Frame header: magic, descriptor, optional window byte, dictionary id,
        // then the content size. Changing the size keeps block framing intact,
        // so only decoding itself can detect the mismatch.
        let descriptor = corrupted[4];
        let single_segment = (descriptor >> 5) & 1 == 1;
        let dictionary_bytes = [0, 1, 2, 4][usize::from(descriptor & 3)];
        let content_size_offset = 5 + usize::from(!single_segment) + dictionary_bytes;
        corrupted[content_size_offset] = corrupted[content_size_offset].wrapping_add(1);
        assert_ne!(
            zstd::zstd_safe::get_frame_content_size(&corrupted).unwrap(),
            declared
        );

        let error = try_decompress_text(corrupted).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("failed to decompress stored chunk")
        );
    }

    #[test]
    fn oversized_frame_is_capped() {
        let oversized = vec![b'a'; MAX_DECOMPRESSED_TEXT_BYTES as usize + 1];
        let compressed = zstd::bulk::compress(&oversized, 1).unwrap();

        let error = try_decompress_text(compressed).unwrap_err();
        assert!(error.to_string().contains("exceeds"));
    }
}
