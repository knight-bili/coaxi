//! One bounded scan owns a read's digest, classification and selected range.
//! Every request still hashes the source: metadata alone is not content identity.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path, sync::Arc};

const CHUNK_BYTES: usize = 64 * 1024;

pub(super) struct Analysis {
    pub generation: String,
    text: bool,
    block_lines: Vec<u64>,
}

#[derive(Clone, Copy)]
pub(super) enum Selection {
    All,
    Metadata,
    Digest,
    Page { offset: u64, maximum: u64 },
    Line { requested: u64 },
}
pub(super) struct Content {
    pub bytes: Vec<u8>,
    pub prefix: Vec<u8>,
    pub size: usize,
    pub version: Value,
    pub generation: String,
    pub text: bool,
    pub has_nul: bool,
    pub line: u64,
    pub line_offset: usize,
    pub analysis: Option<Arc<Analysis>>,
}

// UTF-8 validation carries at most one incomplete scalar across read boundaries.
#[derive(Default)]
struct TextCheck {
    tail: Vec<u8>,
    invalid: bool,
}
impl TextCheck {
    fn push(&mut self, mut bytes: &[u8]) {
        if self.invalid {
            return;
        }
        if !self.tail.is_empty() {
            let width = if self.tail[0] < 0xe0 {
                2
            } else if self.tail[0] < 0xf0 {
                3
            } else {
                4
            };
            let take = (width - self.tail.len()).min(bytes.len());
            self.tail.extend_from_slice(&bytes[..take]);
            bytes = &bytes[take..];
            if self.tail.len() < width {
                return;
            }
            if std::str::from_utf8(&self.tail).is_err() {
                self.invalid = true;
                return;
            }
            self.tail.clear();
        }
        if let Err(error) = std::str::from_utf8(bytes) {
            if error.error_len().is_some() {
                self.invalid = true;
            } else {
                self.tail.extend_from_slice(&bytes[error.valid_up_to()..]);
            }
        }
    }
}

pub(super) async fn read(
    path: &Path,
    selection: Selection,
    permit: Option<Arc<tokio::sync::OwnedSemaphorePermit>>,
) -> Result<Content> {
    read_known(path, selection, None, permit).await
}
pub(super) async fn read_known(
    path: &Path,
    selection: Selection,
    known: Option<Arc<Analysis>>,
    permit: Option<Arc<tokio::sync::OwnedSemaphorePermit>>,
) -> Result<Content> {
    let path = path.to_path_buf();
    // Keep disk I/O and hashing off the async executor, without scheduling one
    // blocking filesystem task for each small chunk.
    tokio::task::spawn_blocking(move || {
        // An HTTP cancellation cannot release capacity while its blocking scan
        // is still running. No handle publication is detached from the request.
        let _permit = permit;
        scan(&path, selection, known.as_deref())
    })
    .await?
}
fn scan(path: &Path, selection: Selection, known: Option<&Analysis>) -> Result<Content> {
    let mut file =
        coaxi_core::local_attachment_snapshot::open_resolved_file_without_following(path)?;
    let meta = file.metadata()?;
    ensure!(meta.is_file(), "not_regular_file");
    ensure!(
        meta.len() <= crate::uploads::MAX_BYTES as u64,
        "file_too_large"
    );
    let mut bytes = Vec::with_capacity(match selection {
        Selection::All => meta.len() as usize,
        Selection::Page { maximum, .. } => maximum.min(256 * 1024) as usize + 3,
        _ => 0,
    });
    let mut prefix = Vec::with_capacity(12);
    let mut digest = Sha256::new();
    let mut text = TextCheck::default();
    let mut has_nul = false;
    let mut size = 0usize;
    let block = if let Some(known) = known {
        match selection {
            Selection::Page { offset, .. } => (usize::try_from(offset).context("read_failed")?
                / CHUNK_BYTES)
                .min(known.block_lines.len().saturating_sub(1)),
            Selection::Line { requested } => known
                .block_lines
                .partition_point(|&line| line < requested)
                .saturating_sub(1),
            _ => 0,
        }
    } else {
        0
    };
    let scan_start = block * CHUNK_BYTES;
    let mut line = known
        .and_then(|known| known.block_lines.get(block))
        .copied()
        .unwrap_or(1);
    let mut line_offset = scan_start;
    let mut block_lines = Vec::new();
    let mut chunk = [0u8; CHUNK_BYTES];
    loop {
        let mut count = 0;
        while count < chunk.len() {
            match file.read(&mut chunk[count..]) {
                Ok(0) => break,
                Ok(read) => count += read,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        if count == 0 {
            break;
        }
        let chunk = &chunk[..count];
        ensure!(size + count <= crate::uploads::MAX_BYTES, "file_too_large");
        digest.update(chunk);
        prefix.extend_from_slice(&chunk[..count.min(12 - prefix.len())]);
        if known.is_none()
            && matches!(
                selection,
                Selection::Metadata | Selection::Page { .. } | Selection::Line { .. }
            )
        {
            text.push(chunk);
            if matches!(selection, Selection::Metadata) {
                has_nul |= memchr::memchr(0, chunk).is_some();
            }
        }
        match selection {
            Selection::All => bytes.extend_from_slice(chunk),
            Selection::Metadata => {
                block_lines.push(line);
                line += memchr::memchr_iter(b'\n', chunk).count() as u64;
            }
            Selection::Page { offset, maximum } => {
                let offset = usize::try_from(offset).context("read_failed")?;
                let end = offset.saturating_add(maximum.min(256 * 1024) as usize + 3);
                let start = offset.saturating_sub(size).min(count);
                let stop = end.saturating_sub(size).min(count);
                if start < stop {
                    bytes.extend_from_slice(&chunk[start..stop]);
                }
                if size >= scan_start {
                    line += memchr::memchr_iter(
                        b'\n',
                        &chunk[..offset.saturating_sub(size).min(count)],
                    )
                    .count() as u64;
                }
            }
            Selection::Line { requested } if size >= scan_start => {
                for (index, &byte) in chunk.iter().enumerate() {
                    if line >= requested {
                        break;
                    }
                    if byte == b'\n' {
                        line += 1;
                    }
                    line_offset = size + index + 1;
                }
            }
            _ => {}
        }
        size += count;
    }
    let after = file.metadata()?;
    ensure!(
        meta.len() == after.len() && meta.modified()? == after.modified()?,
        "read_failed"
    );
    let version = json!({"size":size,"mtimeMs":meta.modified()?.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()});
    let generation = format!("{:x}", digest.finalize());
    // Cached UTF-8 and line facts are usable only after a fresh full digest
    // matches. An equal size/mtime is never sufficient, including after restart.
    if let Some(known) = known {
        ensure!(generation == known.generation, "read_failed");
    }
    let text = known.map_or(!text.invalid && text.tail.is_empty(), |known| known.text);
    let analysis = matches!(selection, Selection::Metadata).then(|| {
        Arc::new(Analysis {
            generation: generation.clone(),
            text,
            block_lines,
        })
    });
    Ok(Content {
        bytes,
        prefix,
        size,
        version,
        generation,
        text,
        has_nul,
        line,
        line_offset,
        analysis,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn read(path: &Path, selection: Selection) -> Result<Content> {
        super::read(path, selection, None).await
    }
    async fn read_known(
        path: &Path,
        selection: Selection,
        known: Option<Arc<Analysis>>,
    ) -> Result<Content> {
        super::read_known(path, selection, known, None).await
    }
    // Owns bounded retention and digest identity at the filesystem scan seam.
    // The existing paging parser test has no I/O, chunk or content-version seam.
    #[tokio::test]
    async fn bounded_scan_preserves_digest_unicode_and_same_metadata_changes() {
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let fixture = Fixture(
            std::fs::canonicalize(std::env::temp_dir())
                .unwrap()
                .join(format!(
                    "coaxi-content-test-{}",
                    crate::new_token().unwrap()
                )),
        );
        let text = format!(
            "{}🌸\n{}",
            "a".repeat(64 * 1024 - 2),
            "你好\n".repeat(300_000)
        );
        let fixed = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
        let write = |bytes: &[u8]| {
            std::fs::write(&fixture.0, bytes).unwrap();
            std::fs::OpenOptions::new()
                .write(true)
                .open(&fixture.0)
                .unwrap()
                .set_times(std::fs::FileTimes::new().set_modified(fixed))
                .unwrap();
        };
        write(text.as_bytes());
        let metadata = read(&fixture.0, Selection::Metadata).await.unwrap();
        assert!(metadata.text && !metadata.has_nul);
        assert!(metadata.bytes.is_empty());
        assert_eq!(metadata.prefix.len(), 12);
        assert_eq!(
            metadata.generation,
            format!("{:x}", Sha256::digest(text.as_bytes()))
        );
        let page = read(
            &fixture.0,
            Selection::Page {
                offset: 64 * 1024 - 2,
                maximum: 1,
            },
        )
        .await
        .unwrap();
        assert!(page.text);
        assert_eq!(page.bytes, "🌸".as_bytes());
        assert_eq!(page.line, 1);
        assert_eq!(page.generation, metadata.generation);
        let line = read(&fixture.0, Selection::Line { requested: 2 })
            .await
            .unwrap();
        assert!(line.bytes.is_empty());
        assert_eq!(line.line_offset, 64 * 1024 + 3);
        assert_eq!(line.line, 2);
        let digest = read(&fixture.0, Selection::Digest).await.unwrap();
        assert!(digest.bytes.is_empty());
        assert_eq!(digest.generation, metadata.generation);
        let analysis = metadata.analysis.clone().unwrap();
        assert!(analysis.block_lines.len() <= crate::uploads::MAX_BYTES.div_ceil(CHUNK_BYTES));
        // Cached facts must produce the same byte/line results at boundaries,
        // after the first chunk, and beyond the last line.
        for selection in [
            Selection::Page {
                offset: 0,
                maximum: 9,
            },
            Selection::Page {
                offset: 64 * 1024 - 2,
                maximum: 1,
            },
            Selection::Page {
                offset: 2 * 64 * 1024 + 2,
                maximum: 256 * 1024,
            },
            Selection::Line { requested: 1 },
            Selection::Line { requested: 2 },
            Selection::Line { requested: 250_000 },
            Selection::Line { requested: 400_000 },
        ] {
            let plain = read(&fixture.0, selection).await.unwrap();
            let cached = read_known(&fixture.0, selection, Some(analysis.clone()))
                .await
                .unwrap();
            assert_eq!(cached.bytes, plain.bytes);
            assert_eq!(cached.line, plain.line);
            if matches!(selection, Selection::Line { .. }) {
                assert_eq!(cached.line_offset, plain.line_offset);
            }
            assert_eq!(cached.text, plain.text);
        }
        let mut changed = text.into_bytes();
        changed[0] = b'b';
        write(&changed);
        let changed = read(&fixture.0, Selection::Metadata).await.unwrap();
        assert_eq!(changed.version, metadata.version);
        assert_ne!(changed.generation, metadata.generation);
        assert!(
            read_known(
                &fixture.0,
                Selection::Page {
                    offset: 0,
                    maximum: 9
                },
                Some(analysis)
            )
            .await
            .is_err()
        );
        write(b"valid text followed by incomplete UTF-8\xf0\x9f");
        assert!(!read(&fixture.0, Selection::Metadata).await.unwrap().text);
    }
}
