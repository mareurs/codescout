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
