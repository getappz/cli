//! Fixed-capacity buffer that retains the head and tail of a byte stream.
//!
//! When the total input exceeds the capacity, the buffer keeps:
//! - The first `capacity / 2` bytes (head)
//! - The last `capacity / 2` bytes (tail, rotating ring buffer)
//!
//! This is useful for capturing tool output without OOM on large projects,
//! while still preserving the most diagnostic information (early lines for
//! context, late lines for the most recent output).
//!
//! Ported from OpenAI Codex `head_tail_buffer.rs`.

/// A buffer that keeps the first N/2 and last N/2 bytes of a stream.
///
/// Default capacity: 1 MiB.
#[derive(Debug, Clone)]
pub struct HeadTailBuffer {
    /// Head portion: filled sequentially until full.
    head: Vec<u8>,
    /// Tail portion: ring buffer, newest data overwrites oldest.
    tail: Vec<u8>,
    /// Maximum size of the head portion.
    head_cap: usize,
    /// Maximum size of the tail portion.
    tail_cap: usize,
    /// Write position in the tail ring buffer.
    tail_write_pos: usize,
    /// Whether the tail has wrapped at least once.
    tail_wrapped: bool,
    /// Whether the head is full (we've switched to tail mode).
    head_full: bool,
    /// Total bytes written (including omitted).
    total_bytes: usize,
}

impl HeadTailBuffer {
    /// Default capacity: 1 MiB.
    pub const DEFAULT_CAPACITY: usize = 1024 * 1024;

    /// Create a new buffer with the given total capacity.
    ///
    /// Capacity is split 50/50 between head and tail.
    pub fn new(capacity: usize) -> Self {
        let head_cap = capacity / 2;
        let tail_cap = capacity - head_cap;
        Self {
            head: Vec::with_capacity(head_cap.min(8192)),
            tail: vec![0u8; tail_cap],
            head_cap,
            tail_cap,
            tail_write_pos: 0,
            tail_wrapped: false,
            head_full: false,
            total_bytes: 0,
        }
    }

    /// Create a buffer with the default 1 MiB capacity.
    pub fn with_default_capacity() -> Self {
        Self::new(Self::DEFAULT_CAPACITY)
    }

    /// Write bytes into the buffer.
    pub fn write(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        self.total_bytes += data.len();

        if !self.head_full {
            let head_remaining = self.head_cap - self.head.len();
            if data.len() <= head_remaining {
                // Fits entirely in head.
                self.head.extend_from_slice(data);
            } else {
                // Fill head, rest goes to tail.
                self.head.extend_from_slice(&data[..head_remaining]);
                self.head_full = true;
                self.write_tail(&data[head_remaining..]);
            }
        } else {
            self.write_tail(data);
        }
    }

    /// Write data to the tail ring buffer.
    fn write_tail(&mut self, data: &[u8]) {
        if self.tail_cap == 0 {
            return;
        }
        for &byte in data {
            self.tail[self.tail_write_pos] = byte;
            self.tail_write_pos = (self.tail_write_pos + 1) % self.tail_cap;
            if self.tail_write_pos == 0 {
                self.tail_wrapped = true;
            }
        }
    }

    /// Total bytes that have been written to the buffer.
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Number of bytes that were omitted (not retained).
    pub fn omitted_bytes(&self) -> usize {
        let retained = self.head.len() + self.tail_len();
        self.total_bytes.saturating_sub(retained)
    }

    /// Current length of valid tail data.
    fn tail_len(&self) -> usize {
        if !self.head_full {
            0
        } else if self.tail_wrapped {
            self.tail_cap
        } else {
            self.tail_write_pos
        }
    }

    /// Returns true if any bytes were omitted.
    pub fn was_truncated(&self) -> bool {
        self.omitted_bytes() > 0
    }

    /// Assemble the final output as a string (lossy UTF-8 conversion).
    ///
    /// If bytes were omitted, a truncation marker is inserted.
    pub fn to_string_lossy(&self) -> String {
        let head_str = String::from_utf8_lossy(&self.head);

        if !self.head_full || self.tail_len() == 0 {
            return head_str.into_owned();
        }

        let tail_bytes = self.tail_bytes();
        let tail_str = String::from_utf8_lossy(&tail_bytes);

        let omitted = self.omitted_bytes();
        if omitted > 0 {
            format!(
                "{}\n\n... ({} bytes omitted) ...\n\n{}",
                head_str, omitted, tail_str
            )
        } else {
            format!("{}{}", head_str, tail_str)
        }
    }

    /// Get the tail bytes in order.
    fn tail_bytes(&self) -> Vec<u8> {
        if !self.head_full {
            return Vec::new();
        }

        if self.tail_wrapped {
            // Ring buffer: data starts at write_pos, wraps around.
            let mut result = Vec::with_capacity(self.tail_cap);
            result.extend_from_slice(&self.tail[self.tail_write_pos..]);
            result.extend_from_slice(&self.tail[..self.tail_write_pos]);
            result
        } else {
            self.tail[..self.tail_write_pos].to_vec()
        }
    }
}

impl Default for HeadTailBuffer {
    fn default() -> Self {
        Self::with_default_capacity()
    }
}

impl std::fmt::Display for HeadTailBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_input_fits_in_head() {
        let mut buf = HeadTailBuffer::new(100);
        buf.write(b"hello world");
        assert_eq!(buf.to_string_lossy(), "hello world");
        assert!(!buf.was_truncated());
        assert_eq!(buf.omitted_bytes(), 0);
    }

    #[test]
    fn large_input_truncates_middle() {
        let mut buf = HeadTailBuffer::new(20);
        // Head: 10 bytes, Tail: 10 bytes
        buf.write(b"AAAAAAAAAA"); // 10 bytes - fills head exactly
        buf.write(b"BBBBBBBBBB"); // 10 bytes - goes to tail
        buf.write(b"CCCCCCCCCC"); // 10 bytes - overwrites tail

        assert!(buf.was_truncated());
        let output = buf.to_string_lossy();
        assert!(output.starts_with("AAAAAAAAAA"));
        assert!(output.contains("omitted"));
        assert!(output.ends_with("CCCCCCCCCC"));
    }

    #[test]
    fn exact_capacity_no_truncation() {
        let mut buf = HeadTailBuffer::new(20);
        buf.write(b"AAAAAAAAAA"); // fills head (10)
        buf.write(b"BBBBBBBBBB"); // fills tail (10)

        assert!(!buf.was_truncated());
        assert_eq!(buf.to_string_lossy(), "AAAAAAAAAABBBBBBBBBB");
    }
}
