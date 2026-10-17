use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use super::{FileSearchResult, SearchOptions, literal_hits_for_file};

// A sum of 1.0f32 stops increasing at this value. Preserve that score while
// ranking files by their exact match counts.
const MAX_LITERAL_FILE_SCORE: usize = 1 << 24;
const DEFAULT_LITERAL_PREVIEWS_PER_FILE: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FileSearchMode {
    Files,
    FileNames,
    SingleFileName,
}

#[derive(Debug)]
struct RankedFile<'a> {
    relative: &'a PathBuf,
    absolute: &'a PathBuf,
    matches: usize,
    matches_are_lower_bound: bool,
}

impl RankedFile<'_> {
    fn score(&self) -> usize {
        self.matches.min(MAX_LITERAL_FILE_SCORE)
    }
}

impl Ord for RankedFile<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        // The heap keeps the worst selected file at the top.
        other
            .matches
            .cmp(&self.matches)
            .then_with(|| self.relative.cmp(other.relative))
    }
}

impl PartialOrd for RankedFile<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for RankedFile<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for RankedFile<'_> {}

fn insert_ranked<'a>(
    heap: &mut BinaryHeap<RankedFile<'a>>,
    file: RankedFile<'a>,
    max_files: usize,
) {
    heap.push(file);
    if heap.len() > max_files {
        heap.pop();
    }
}

pub(super) fn search_files(
    root: &Path,
    query_lower: &str,
    context: usize,
    max_files: usize,
    paths: &[(PathBuf, PathBuf)],
    options: &SearchOptions,
    mode: FileSearchMode,
) -> anyhow::Result<Vec<FileSearchResult>> {
    if max_files == 0 || query_lower.contains('\n') {
        return Ok(Vec::new());
    }
    let files = crate::workspace_file::RootHandle::new(root);
    // An explicit unlimited request needs every preview, so one pass suffices.
    if mode == FileSearchMode::Files && max_files == usize::MAX {
        let results = paths
            .par_iter()
            .filter_map(|(relative, absolute)| {
                let hits = literal_hits_for_file(
                    &files,
                    relative,
                    absolute,
                    query_lower,
                    context,
                    usize::MAX,
                    options,
                );
                (!hits.is_empty()).then(|| FileSearchResult {
                    file_path: relative.clone(),
                    total_score: hits.len().min(MAX_LITERAL_FILE_SCORE) as f32,
                    hit_count: hits.len(),
                    more_hits_in_file: 0,
                    hit_count_is_lower_bound: false,
                    hits,
                })
            })
            .collect();
        return Ok(if options.is_cancelled() {
            Vec::new()
        } else {
            results
        });
    }

    let matcher = LineMatcher::new(query_lower);
    let candidates = paths.par_iter().filter_map(|(relative, absolute)| {
        if options.is_cancelled() {
            return None;
        }
        let file = files.open(absolute).ok()?;
        let count = matcher
            .count_lines(file, options, mode == FileSearchMode::SingleFileName)
            .ok()??;
        (count.matches > 0).then_some(RankedFile {
            relative,
            absolute,
            matches: count.matches,
            matches_are_lower_bound: count.is_lower_bound,
        })
    });

    let mut selected = if max_files >= paths.len() {
        candidates.collect::<Vec<_>>()
    } else {
        candidates
            .fold(BinaryHeap::new, |mut heap, file| {
                insert_ranked(&mut heap, file, max_files);
                heap
            })
            .reduce(BinaryHeap::new, |mut left, right| {
                for file in right {
                    insert_ranked(&mut left, file, max_files);
                }
                left
            })
            .into_vec()
    };
    if options.is_cancelled() {
        return Ok(Vec::new());
    }
    selected.sort_unstable();

    if mode != FileSearchMode::Files {
        return Ok(selected
            .into_iter()
            .map(|file| FileSearchResult {
                file_path: file.relative.clone(),
                total_score: file.score() as f32,
                hit_count: file.matches,
                more_hits_in_file: file.matches,
                hit_count_is_lower_bound: file.matches_are_lower_bound,
                hits: Vec::new(),
            })
            .collect());
    }

    let results = selected
        .into_par_iter()
        .filter_map(|file| {
            let hits = literal_hits_for_file(
                &files,
                file.relative,
                file.absolute,
                query_lower,
                context,
                DEFAULT_LITERAL_PREVIEWS_PER_FILE,
                options,
            );
            if hits.is_empty() {
                return None;
            }
            let match_count = file.matches.max(hits.len());
            Some(FileSearchResult {
                file_path: file.relative.clone(),
                total_score: match_count.min(MAX_LITERAL_FILE_SCORE) as f32,
                hit_count: match_count,
                more_hits_in_file: match_count - hits.len(),
                hit_count_is_lower_bound: false,
                hits,
            })
        })
        .collect();
    Ok(if options.is_cancelled() {
        Vec::new()
    } else {
        results
    })
}

/// Counts matching lines without retaining file contents or preview strings.
struct LineMatcher<'a> {
    needle: &'a [u8],
    prefix: Vec<usize>,
}

struct LineCount {
    matches: usize,
    is_lower_bound: bool,
}

impl<'a> LineMatcher<'a> {
    fn new(query_lower: &'a str) -> Self {
        let needle = query_lower.as_bytes();
        let mut prefix = vec![0; needle.len()];
        for index in 1..needle.len() {
            let mut matched = prefix[index - 1];
            while matched > 0 && needle[index] != needle[matched] {
                matched = prefix[matched - 1];
            }
            if needle[index] == needle[matched] {
                matched += 1;
            }
            prefix[index] = matched;
        }
        Self { needle, prefix }
    }

    /// Returns `None` for binary files and cancelled searches.
    fn count_lines(
        &self,
        mut file: impl Read,
        options: &SearchOptions,
        stop_after_first_match: bool,
    ) -> io::Result<Option<LineCount>> {
        let mut counter = LineCounter::new(self);
        let mut buffer = [0u8; 16 * 1024 + 4];
        let mut carry = 0;
        loop {
            if options.is_cancelled() {
                return Ok(None);
            }
            let read = match file.read(&mut buffer[carry..]) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => result?,
            };
            let end = carry + read;
            let mut offset = 0;
            carry = 0;
            while offset < end {
                match std::str::from_utf8(&buffer[offset..end]) {
                    Ok(text) => {
                        if !counter.text(text) {
                            return Ok(None);
                        }
                        break;
                    }
                    Err(error) => {
                        let valid_end = offset + error.valid_up_to();
                        let valid = std::str::from_utf8(&buffer[offset..valid_end]).unwrap();
                        if !counter.text(valid) {
                            return Ok(None);
                        }
                        offset = valid_end;
                        if let Some(invalid_bytes) = error.error_len() {
                            if !counter.character('\u{fffd}') {
                                return Ok(None);
                            }
                            offset += invalid_bytes;
                        } else if read == 0 {
                            if !counter.character('\u{fffd}') {
                                return Ok(None);
                            }
                            break;
                        } else {
                            carry = end - offset;
                            buffer.copy_within(offset..end, 0);
                            break;
                        }
                    }
                }
            }
            // One filename does not need a full count for ranking. Still read
            // the complete binary-sniff prefix before accepting that match.
            if read > 0
                && stop_after_first_match
                && counter.sniffed_bytes >= crate::chunking::TEXT_SNIFF_BYTES
                && (counter.lines > 0 || counter.matched_line)
            {
                return Ok(Some(LineCount {
                    matches: counter.finish(),
                    is_lower_bound: true,
                }));
            }
            if read == 0 {
                break;
            }
        }
        Ok(Some(LineCount {
            matches: counter.finish(),
            is_lower_bound: false,
        }))
    }
}

struct LineCounter<'a> {
    matcher: &'a LineMatcher<'a>,
    matched: usize,
    matched_line: bool,
    lines: usize,
    sniffed_bytes: usize,
}

impl<'a> LineCounter<'a> {
    fn new(matcher: &'a LineMatcher<'a>) -> Self {
        Self {
            matcher,
            matched: 0,
            matched_line: false,
            lines: 0,
            sniffed_bytes: 0,
        }
    }

    fn text(&mut self, text: &str) -> bool {
        if text.is_ascii() {
            for byte in text.bytes() {
                if !self.character(char::from(byte)) {
                    return false;
                }
            }
        } else {
            for character in text.chars() {
                if !self.character(character) {
                    return false;
                }
            }
        }
        true
    }

    fn character(&mut self, character: char) -> bool {
        if self.sniffed_bytes < crate::chunking::TEXT_SNIFF_BYTES && character == '\0' {
            return false;
        }
        self.sniffed_bytes = self
            .sniffed_bytes
            .saturating_add(character.len_utf8())
            .min(crate::chunking::TEXT_SNIFF_BYTES);
        if character == '\n' {
            self.lines += usize::from(self.matched_line);
            self.matched = 0;
            self.matched_line = false;
        } else if !self.matched_line {
            if character.is_ascii() {
                self.byte((character as u8).to_ascii_lowercase());
            } else {
                for lower in character.to_lowercase() {
                    let lower = if lower == 'ς' { 'σ' } else { lower };
                    let mut bytes = [0; 4];
                    for byte in lower.encode_utf8(&mut bytes).bytes() {
                        self.byte(byte);
                        if self.matched_line {
                            break;
                        }
                    }
                    if self.matched_line {
                        break;
                    }
                }
            }
        }
        true
    }

    fn byte(&mut self, byte: u8) {
        while self.matched > 0 && self.matcher.needle[self.matched] != byte {
            self.matched = self.matcher.prefix[self.matched - 1];
        }
        if self.matcher.needle[self.matched] == byte {
            self.matched += 1;
        }
        self.matched_line = self.matched == self.matcher.needle.len();
    }

    fn finish(self) -> usize {
        self.lines + usize::from(self.matched_line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::fold_literal_case;

    #[test]
    fn bounded_file_selection_uses_exact_large_match_counts() {
        let first = PathBuf::from("a.txt");
        let second = PathBuf::from("b.txt");
        let mut selected = BinaryHeap::new();
        for (path, matches) in [(&first, 1 << 24), (&second, (1 << 24) + 1)] {
            insert_ranked(
                &mut selected,
                RankedFile {
                    relative: path,
                    absolute: path,
                    matches,
                    matches_are_lower_bound: false,
                },
                1,
            );
        }
        assert_eq!(selected.pop().unwrap().relative, &second);
    }

    struct ChunkedReader<'a> {
        bytes: &'a [u8],
        chunk_size: usize,
    }

    impl Read for ChunkedReader<'_> {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            let size = output.len().min(self.chunk_size).min(self.bytes.len());
            output[..size].copy_from_slice(&self.bytes[..size]);
            self.bytes = &self.bytes[size..];
            Ok(size)
        }
    }

    #[test]
    fn streaming_line_counts_match_lossy_unicode_search_across_read_boundaries() {
        let mut boundary = vec![b'x'; 16_387];
        boundary.extend_from_slice("Kelvin\nabaBABacA\naxİ\r\n".as_bytes());
        let mut binary = vec![b'x'; crate::chunking::TEXT_SNIFF_BYTES - 1];
        binary.extend_from_slice(b"\0needle\n");
        let mut lossy = vec![0xff; 4_096];
        lossy.extend_from_slice(b"\0needle\n");
        let cases = [
            b"ABAbaBACa\r\nneedLE\nneedle needle\ninvalid \xffx\n".to_vec(),
            "Kelvin\nΛΟΓΟΣ\nλογος\nİstanbul\naxİ\n".as_bytes().to_vec(),
            boundary,
            binary,
            lossy,
        ];
        let queries = [
            "needle",
            "ababaca",
            "kelvin",
            "λογοσ",
            "istanbul",
            "axi",
            "\u{fffd}x",
        ];
        for bytes in cases {
            let content = String::from_utf8_lossy(&bytes);
            let binary = content.as_bytes()[..content.len().min(crate::chunking::TEXT_SNIFF_BYTES)]
                .contains(&0);
            for query in queries {
                let query_lower = fold_literal_case(query);
                let expected = (!binary).then(|| {
                    content
                        .lines()
                        .filter(|line| fold_literal_case(line).contains(&query_lower))
                        .count()
                });
                let matcher = LineMatcher::new(&query_lower);
                for chunk_size in [1, 2, 3, 7, 16_384] {
                    let reader = ChunkedReader {
                        bytes: &bytes,
                        chunk_size,
                    };
                    let actual = matcher
                        .count_lines(reader, &SearchOptions::default(), false)
                        .unwrap()
                        .map(|count| count.matches);
                    assert_eq!(actual, expected, "{query:?}, chunk size {chunk_size}");
                }
            }
        }
    }

    #[test]
    fn single_filename_search_stops_after_matching_and_sniffing_for_binary() {
        let mut bytes = b"needle\n".to_vec();
        bytes.extend(std::iter::repeat_n(b'x', 100_000));
        let matcher = LineMatcher::new("needle");
        let mut reader = io::Cursor::new(&bytes);
        let count = matcher
            .count_lines(&mut reader, &SearchOptions::default(), true)
            .unwrap()
            .unwrap();
        assert_eq!(count.matches, 1);
        assert!(count.is_lower_bound);
        assert!(reader.position() < bytes.len() as u64);

        let mut binary = b"needle\n".to_vec();
        binary.extend(std::iter::repeat_n(
            b'x',
            crate::chunking::TEXT_SNIFF_BYTES - binary.len() - 1,
        ));
        binary.push(0);
        binary.extend(std::iter::repeat_n(b'x', 100_000));
        assert!(
            matcher
                .count_lines(&binary[..], &SearchOptions::default(), true)
                .unwrap()
                .is_none()
        );
    }
}
