//! Server-sent events (`text/event-stream`) and newline-delimited JSON, read line by line.

use std::io::{self, BufRead};

/// Longest line or event accepted from a stream, so a broken server cannot exhaust memory.
pub(crate) const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;

/// One dispatched server-sent event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SseEvent {
    /// The `event:` field, when the server named one.
    pub event: Option<String>,
    /// The `data:` lines, joined with `\n`.
    pub data: String,
}

/// Builds [`SseEvent`]s from lines, as the HTML standard describes: `data:` lines accumulate,
/// a blank line dispatches, `:` lines are comments, and other fields are ignored.
#[derive(Debug, Default)]
pub struct SseParser {
    event: Option<String>,
    data: String,
    has_data: bool,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds one line, without its line ending. Returns an event when the line completes one.
    pub fn push_line(&mut self, line: &str) -> Option<SseEvent> {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            return self.dispatch();
        }
        if line.starts_with(':') {
            return None;
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "data" => {
                if self.has_data {
                    self.data.push('\n');
                }
                if self.data.len().saturating_add(value.len()) <= MAX_LINE_BYTES {
                    self.data.push_str(value);
                }
                self.has_data = true;
            }
            "event" => self.event = Some(value.to_owned()),
            _ => {}
        }
        None
    }

    /// Dispatches a final event the stream ended without a blank line after.
    pub fn finish(&mut self) -> Option<SseEvent> {
        self.dispatch()
    }

    fn dispatch(&mut self) -> Option<SseEvent> {
        let event = self.event.take();
        if !self.has_data {
            return None;
        }
        self.has_data = false;
        Some(SseEvent { event, data: std::mem::take(&mut self.data) })
    }
}

/// Reads one line (up to `\n`, which is dropped) into `out`, refusing lines over
/// [`MAX_LINE_BYTES`]. Returns `Ok(false)` at the end of the stream.
pub(crate) fn read_line(reader: &mut dyn BufRead, out: &mut Vec<u8>) -> io::Result<bool> {
    out.clear();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(!out.is_empty());
        }
        if let Some(pos) = available.iter().position(|b| *b == b'\n') {
            out.extend_from_slice(&available[..pos]);
            reader.consume(pos + 1);
            return Ok(true);
        }
        let len = available.len();
        out.extend_from_slice(available);
        reader.consume(len);
        if out.len() > MAX_LINE_BYTES {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "a streamed line is too long"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Vec<SseEvent> {
        let mut p = SseParser::new();
        let mut out: Vec<SseEvent> = text.split('\n').filter_map(|l| p.push_line(l)).collect();
        out.extend(p.finish());
        out
    }

    #[test]
    fn events_comments_and_multiline_data() {
        let events = parse(": keep-alive\r\ndata: {\"a\":1}\r\n\r\nevent: done\ndata: line one\ndata:line two\n\n");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], SseEvent { event: None, data: "{\"a\":1}".into() });
        assert_eq!(events[1], SseEvent { event: Some("done".into()), data: "line one\nline two".into() });
    }

    #[test]
    fn blank_lines_without_data_dispatch_nothing_and_a_tail_is_flushed() {
        assert!(parse("\n\nretry: 10\n\n").is_empty());
        assert_eq!(parse("data: tail").len(), 1);
        assert_eq!(parse("data\n\n")[0].data, "");
    }

    #[test]
    fn read_line_splits_and_bounds() {
        let mut input: &[u8] = b"one\ntwo\r\nthree";
        let mut buf = Vec::new();
        let mut lines = Vec::new();
        while read_line(&mut input, &mut buf).unwrap() {
            lines.push(String::from_utf8_lossy(&buf).into_owned());
        }
        assert_eq!(lines, ["one", "two\r", "three"]);
        let long = vec![b'x'; MAX_LINE_BYTES + 10];
        let mut reader: &[u8] = &long;
        assert!(read_line(&mut reader, &mut buf).is_err());
    }
}

/// Whether a line handler wants more lines.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Flow {
    More,
    Done,
}

/// Feeds a streamed body to `on_line` line by line, stopping on cancel, on `Flow::Done` or at
/// the end of the stream. Returns whether `on_line` asked to stop (`true`) or the stream simply
/// ended (`false`).
pub(crate) fn for_each_line(
    reader: impl io::Read,
    cancel: &std::sync::atomic::AtomicBool,
    mut on_line: impl FnMut(&str) -> Result<Flow, craft_ai_core::AiError>,
) -> Result<bool, craft_ai_core::AiError> {
    let mut reader = io::BufReader::new(reader);
    let mut buf = Vec::new();
    loop {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(craft_ai_core::AiError::cancelled());
        }
        let more = read_line(&mut reader, &mut buf)
            .map_err(|e| craft_ai_core::AiError::new(craft_ai_core::ErrorKind::Network, format!("the stream broke off: {e}")))?;
        if !more {
            return Ok(false);
        }
        let line = String::from_utf8_lossy(&buf);
        if on_line(&line)? == Flow::Done {
            return Ok(true);
        }
    }
}
