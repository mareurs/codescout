//! JSON-RPC transport layer for LSP communication.
//!
//! Implements the base protocol from the LSP specification:
//! Content-Length header framing over byte streams.

use anyhow::{bail, Context, Result};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

/// Read a single JSON-RPC message from an async buffered reader.
///
/// Parses the `Content-Length` header, reads the body, and returns parsed JSON.
///
/// # Cancel safety
///
/// NOT cancel-safe. The `Content-Length` it has parsed and the body bytes it has read live in
/// locals across `.await` points, so a future dropped part-way (a losing `select!` arm, a
/// `timeout`) loses what it had consumed from the reader and the next read starts in the middle
/// of a frame. Await it to completion, or after a drop stop using the reader. To race it against
/// other work, use [`read_message_returning_reader`] on a pinned future that outlives the
/// `select!`. The mux event loop (`lsp/mux/process.rs`) is the one caller that races it.
pub async fn read_message<R: AsyncBufReadExt + Unpin>(reader: &mut R) -> Result<Value> {
    let mut content_length: Option<usize> = None;
    let mut header = String::new();

    // Read headers until empty line
    loop {
        header.clear();
        let bytes_read = reader.read_line(&mut header).await?;
        if bytes_read == 0 {
            bail!("EOF while reading message headers");
        }
        let trimmed = header.trim();
        if trimmed.is_empty() {
            break;
        }
        // Robust header parse: case-insensitive name, any whitespace around ':'.
        // LSP spec requires a single space after ':' but some implementations
        // drift; splitn+trim is forgiving without being permissive.
        if let Some((name, value)) = trimmed.split_once(':') {
            if name.trim().eq_ignore_ascii_case("Content-Length") {
                content_length = Some(value.trim().parse().context("invalid Content-Length")?);
            }
        }
        // Ignore Content-Type and other headers
    }

    let length = content_length.context("missing Content-Length header")?;
    // 16 MiB per-message cap. LSP responses (documentSymbol, hover, references)
    // rarely exceed the KB-MB range even on huge files; a tight cap bounds
    // worst-case allocation per mux client and neutralizes oversized-header DOS.
    // cap-class: NOT_A_CAP — per-message allocation ceiling; an oversized frame bails with an error, nothing is truncated
    const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;
    if length > MAX_MESSAGE_SIZE {
        bail!(
            "Content-Length {} exceeds maximum allowed size of {} bytes",
            length,
            MAX_MESSAGE_SIZE,
        );
    }
    let mut body = vec![0u8; length];
    reader
        .read_exact(&mut body)
        .await
        .context("EOF while reading message body")?;

    serde_json::from_slice(&body).context("invalid JSON in message body")
}
/// [`read_message`] for a caller that must race the read against other work in a `select!`.
///
/// `read_message` is NOT cancel-safe (see its doc), so a `select!` must never drop it half done.
/// The way to keep it alive is to pin ONE read future outside the loop, poll it by `&mut` in the
/// `select!`, and when it completes start the next read from the reader this function hands back.
/// A plain `read_message(reader)` written inside the `select!` arm is rebuilt, and the half-read
/// frame dropped, on every pass of the loop. Returning the reader is what lets the next future
/// reborrow it: the previous future's borrow is still alive in the pinned slot until it is
/// overwritten by `Pin::set`, and a second `&mut *reader` taken from the outer reference at that
/// point would be rejected by the borrow checker.
///
/// ```ignore
/// let mut read = std::pin::pin!(read_message_returning_reader(&mut *reader));
/// loop {
///     tokio::select! {
///         (r, msg) = &mut read => { read.set(read_message_returning_reader(r)); /* use msg */ }
///         _ = other_work() => { /* the pending read keeps its half-read frame */ }
///     }
/// }
/// ```
pub async fn read_message_returning_reader<R: AsyncBufReadExt + Unpin>(
    reader: &mut R,
) -> (&mut R, Result<Value>) {
    let msg = read_message(reader).await;
    (reader, msg)
}

/// Write a JSON-RPC message to an async writer.
///
/// Serializes the value to JSON, prepends `Content-Length` header, and flushes.
pub async fn write_message<W: AsyncWriteExt + Unpin>(writer: &mut W, msg: &Value) -> Result<()> {
    let body = serde_json::to_string(msg)?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    writer.write_all(header.as_bytes()).await?;
    writer.write_all(body.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn roundtrip_message() {
        let msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "test",
            "params": { "hello": "world" }
        });

        let mut buf = Vec::new();
        write_message(&mut buf, &msg).await.unwrap();

        let mut reader = BufReader::new(buf.as_slice());
        let result = read_message(&mut reader).await.unwrap();
        assert_eq!(result, msg);
    }

    /// Regression for
    /// docs/issues/2026-10-18-read-message-is-not-cancel-safe-and-the-mux-event-loop-cancels-it.md
    ///
    /// The mux event loop (`lsp/mux/process.rs`) races the server read against accept, the
    /// watchdog tick and signals. The read future is pinned outside the loop, so a `select!` that
    /// another arm wins leaves it, and the half-read frame inside it, alive.
    #[tokio::test]
    async fn a_read_dropped_mid_body_does_not_lose_the_bytes_it_had_consumed() {
        let (mut writer, reader) = tokio::io::duplex(4096);
        let mut reader = BufReader::new(reader);
        let first = r#"{"jsonrpc":"2.0","method":"a","params":{"k":"0123456789"}}"#;
        let second = r#"{"jsonrpc":"2.0","method":"b"}"#;
        let frame = |body: &str| format!("Content-Length: {}\r\n\r\n{body}", body.len());
        let first_frame = frame(first);
        let cut = first_frame.len() - 10;
        writer
            .write_all(&first_frame.as_bytes()[..cut])
            .await
            .unwrap();

        let mut read = std::pin::pin!(read_message_returning_reader(&mut reader));

        // The read polls first, consumes the header and part of the body, and is pending; the
        // other arm is ready, so `select!` takes it and leaves the read where it is.
        tokio::select! {
            biased;
            _ = &mut read => panic!("the frame is incomplete"),
            _ = std::future::ready(()) => {}
        }

        writer
            .write_all(&first_frame.as_bytes()[cut..])
            .await
            .unwrap();
        writer.write_all(frame(second).as_bytes()).await.unwrap();
        let (reader, msg) = read.await;
        assert_eq!(
            msg.as_ref().ok().and_then(|m| m["method"].as_str()),
            Some("a"),
            "the pending read must still deliver the first message whole, got: {msg:?}"
        );
        let next = read_message(reader).await;
        assert_eq!(
            next.as_ref().ok().and_then(|m| m["method"].as_str()),
            Some("b"),
            "the stream must still be framed after the first message, got: {next:?}"
        );
    }
    /// The hazard the pinned future exists to avoid, pinned down so that nobody "simplifies" the
    /// event loop back to `read_message(reader)` inside the `select!`: a read future DROPPED
    /// part-way loses the bytes it had consumed and the next read starts mid-frame.
    #[tokio::test]
    async fn a_read_dropped_mid_body_desynchronises_the_stream() {
        let (mut writer, reader) = tokio::io::duplex(4096);
        let mut reader = BufReader::new(reader);
        let first = r#"{"jsonrpc":"2.0","method":"a","params":{"k":"0123456789"}}"#;
        let second = r#"{"jsonrpc":"2.0","method":"b"}"#;
        let frame = |body: &str| format!("Content-Length: {}\r\n\r\n{body}", body.len());
        let first_frame = frame(first);
        let cut = first_frame.len() - 10;
        writer
            .write_all(&first_frame.as_bytes()[..cut])
            .await
            .unwrap();
        tokio::select! {
            biased;
            _ = read_message(&mut reader) => panic!("the frame is incomplete"),
            _ = std::future::ready(()) => {}
        }
        writer
            .write_all(&first_frame.as_bytes()[cut..])
            .await
            .unwrap();
        writer.write_all(frame(second).as_bytes()).await.unwrap();
        let next = read_message(&mut reader).await;
        assert!(
            next.is_err(),
            "a dropped read is documented as unrecoverable; if this now yields a message, \
             read_message became cancel-safe and its doc and the event loop can be simplified: \
             {next:?}"
        );
    }

    #[tokio::test]
    async fn read_eof_returns_error() {
        let mut reader = BufReader::new(&b""[..]);
        assert!(read_message(&mut reader).await.is_err());
    }

    #[tokio::test]
    async fn read_missing_content_length_errors() {
        let data = b"Content-Type: application/json\r\n\r\n{}";
        let mut reader = BufReader::new(&data[..]);
        assert!(read_message(&mut reader).await.is_err());
    }

    #[tokio::test]
    async fn read_multiple_messages() {
        let mut buf = Vec::new();
        let msg1 = json!({"jsonrpc": "2.0", "id": 1, "result": null});
        let msg2 = json!({"jsonrpc": "2.0", "id": 2, "result": "ok"});
        write_message(&mut buf, &msg1).await.unwrap();
        write_message(&mut buf, &msg2).await.unwrap();

        let mut reader = BufReader::new(buf.as_slice());
        let r1 = read_message(&mut reader).await.unwrap();
        let r2 = read_message(&mut reader).await.unwrap();
        assert_eq!(r1["id"], 1);
        assert_eq!(r2["id"], 2);
    }

    #[tokio::test]
    async fn write_produces_valid_framing() {
        let msg = json!({"test": true});
        let mut buf = Vec::new();
        write_message(&mut buf, &msg).await.unwrap();

        let output = String::from_utf8(buf).unwrap();
        let body = serde_json::to_string(&msg).unwrap();
        let expected = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        assert_eq!(output, expected);
    }

    #[tokio::test]
    async fn rejects_oversized_content_length() {
        let oversized = 32 * 1024 * 1024; // 32 MiB (over the 16 MiB cap)
        let msg = format!("Content-Length: {}\r\n\r\n", oversized);
        let mut reader = BufReader::new(msg.as_bytes());
        let result = read_message(&mut reader).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds"));
    }

    #[tokio::test]
    async fn accepts_normal_content_length() {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#;
        let msg = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let mut reader = BufReader::new(msg.as_bytes());
        let result = read_message(&mut reader).await;
        assert!(result.is_ok());
    }
}
