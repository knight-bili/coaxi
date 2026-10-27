use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Result, anyhow, ensure};
use serde_json::Value;

use super::super::source::NativeDocument;

#[derive(Default, Debug, Clone)]
pub(crate) struct ParseNotes {
    pub(crate) malformed_records: usize,
    pub(crate) incomplete_tail: bool,
    pub(crate) unavailable_children: usize,
    pub(crate) unavailable_advisories: usize,
}

/// UTF-8/JSON records may span any number of verified byte chunks. Only one
/// record is buffered; raw launch metadata never becomes a public response.
pub(in crate::runtime_subagent_session) struct JsonRecords<'a> {
    document: &'a mut NativeDocument,
    stop: &'a AtomicBool,
    notes: &'a mut ParseNotes,
    buffer: Vec<u8>,
    start: usize,
    next_chunk: usize,
    done: bool,
    first_line: bool,
}

impl<'a> JsonRecords<'a> {
    pub(in crate::runtime_subagent_session) fn new(
        document: &'a mut NativeDocument,
        stop: &'a AtomicBool,
        notes: &'a mut ParseNotes,
    ) -> Self {
        Self {
            document,
            stop,
            notes,
            buffer: Vec::new(),
            start: 0,
            next_chunk: 0,
            done: false,
            first_line: true,
        }
    }
}

impl Iterator for JsonRecords<'_> {
    type Item = Result<Value>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        loop {
            if self.stop.load(Ordering::Acquire) {
                self.done = true;
                return Some(Err(anyhow!("native_child.read_cancelled")));
            }
            let newline = self.buffer[self.start..]
                .iter()
                .position(|b| *b == b'\n')
                .map(|at| at + self.start);
            let eof = self.next_chunk == self.document.chunk_count();
            if let Some(end) = newline.or_else(|| eof.then_some(self.buffer.len())) {
                let line = &self.buffer[self.start..end];
                self.start = (end + usize::from(newline.is_some())).min(self.buffer.len());
                if eof && newline.is_none() {
                    self.done = true;
                }
                let first_line = self.first_line;
                self.first_line = false;
                let line = if first_line {
                    line.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(line)
                } else {
                    line
                };
                if line.iter().all(u8::is_ascii_whitespace) {
                    if self.done {
                        return None;
                    }
                    continue;
                }
                match serde_json::from_slice::<Value>(line) {
                    Ok(value) => return Some(Ok(value)),
                    Err(error) if self.done && error.is_eof() => {
                        self.notes.incomplete_tail = true;
                        return None;
                    }
                    Err(_) => {
                        self.notes.malformed_records += 1;
                        if self.done {
                            return None;
                        }
                        continue;
                    }
                }
            }
            if self.start > 0 {
                self.buffer.drain(..self.start);
                self.start = 0;
            }
            match self.document.read_chunk(self.next_chunk) {
                Ok(bytes) => {
                    self.buffer.extend(bytes);
                    self.next_chunk += 1;
                }
                Err(error) => {
                    self.done = true;
                    return Some(Err(error));
                }
            }
        }
    }
}

pub(in crate::runtime_subagent_session) fn json_document(
    document: &mut NativeDocument,
    stop: &AtomicBool,
) -> Result<Value> {
    let mut bytes = Vec::new();
    for index in 0..document.chunk_count() {
        ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
        bytes.extend(document.read_chunk(index)?);
    }
    Ok(serde_json::from_slice(&bytes)?)
}

pub(super) fn text_document(document: &mut NativeDocument, stop: &AtomicBool) -> Result<String> {
    let mut bytes = Vec::new();
    for index in 0..document.chunk_count() {
        ensure!(!stop.load(Ordering::Acquire), "native_child.read_cancelled");
        bytes.extend(document.read_chunk(index)?);
    }
    Ok(String::from_utf8(bytes)?)
}
