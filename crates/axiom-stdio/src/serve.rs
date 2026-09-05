//! NDJSON request/response loop.
//!
//! Each request occupies one line of at most [`super::json::MAX_FRAME_BYTES`]
//! bytes, excluding the newline. Oversized frames are rejected after a bounded
//! read; the remainder of the line is discarded without storing it, then the
//! next request is served. A frame that hits EOF without a newline is treated
//! as the last request.

use std::io::{BufRead, Write};

use axiom_rs::command::{Command, CommandResponse, decode, execute};
use axiom_rs::foundation::Value;
use axiom_rs::runtime::Runtime;

use super::json::{AdapterError, MAX_FRAME_BYTES, decode_value, encode_value, protocol_error_json};

/// Read NDJSON commands from `input` and write responses to `output`.
///
/// Diagnostics are written only to `diag`. Protocol failures still produce a
/// JSON error object on `output`.
///
/// # Errors
///
/// Returns I/O errors from reading or writing. JSON and command failures are
/// encoded as responses, not I/O errors.
pub fn serve<R, W, D>(
    runtime: &Runtime,
    mut input: R,
    mut output: W,
    mut diag: D,
) -> std::io::Result<()>
where
    R: BufRead,
    W: Write,
    D: Write,
{
    loop {
        match read_frame(&mut input)? {
            Frame::Eof => break,
            Frame::Skip => {}
            Frame::TooLarge => {
                write_protocol_error(&mut output, &mut diag, AdapterError::limit_exceeded())?
            }
            Frame::Line(line) => {
                let response = handle_line(runtime, &line, &mut diag)?;
                write_line(&mut output, &response)?;
            }
        }
    }
    Ok(())
}

enum Frame {
    Eof,
    Skip,
    Line(String),
    TooLarge,
}

fn read_frame<R: BufRead>(input: &mut R) -> std::io::Result<Frame> {
    let mut buf = Vec::new();
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            if buf.is_empty() {
                return Ok(Frame::Eof);
            }
            return finish_bytes(buf);
        }
        if let Some(newline) = available.iter().position(|&b| b == b'\n') {
            if buf.len() + newline > MAX_FRAME_BYTES {
                input.consume(newline + 1);
                return Ok(Frame::TooLarge);
            }
            buf.extend_from_slice(&available[..newline]);
            input.consume(newline + 1);
            return finish_bytes(buf);
        }
        if buf.len() + available.len() > MAX_FRAME_BYTES {
            drain_until_newline(input)?;
            return Ok(Frame::TooLarge);
        }
        let n = available.len();
        buf.extend_from_slice(available);
        input.consume(n);
    }
}

fn drain_until_newline<R: BufRead>(input: &mut R) -> std::io::Result<()> {
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            return Ok(());
        }
        if let Some(newline) = available.iter().position(|&b| b == b'\n') {
            input.consume(newline + 1);
            return Ok(());
        }
        let n = available.len();
        input.consume(n);
    }
}

fn finish_bytes(mut buf: Vec<u8>) -> std::io::Result<Frame> {
    if buf.last() == Some(&b'\r') {
        buf.pop();
    }
    if buf.len() > MAX_FRAME_BYTES {
        return Ok(Frame::TooLarge);
    }
    let line = String::from_utf8(buf)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    if line.trim().is_empty() {
        Ok(Frame::Skip)
    } else {
        Ok(Frame::Line(line))
    }
}

fn write_protocol_error<W: Write, D: Write>(
    output: &mut W,
    diag: &mut D,
    error: AdapterError,
) -> std::io::Result<()> {
    writeln!(diag, "axiom-stdio: {}", error.message())?;
    write_line(output, &protocol_error_json(&error))
}

fn write_line<W: Write>(output: &mut W, line: &str) -> std::io::Result<()> {
    output.write_all(line.as_bytes())?;
    output.write_all(b"\n")?;
    output.flush()
}

fn handle_line<D: Write>(runtime: &Runtime, line: &str, diag: &mut D) -> std::io::Result<String> {
    match decode_value(line) {
        Ok(value) => match decode(&value) {
            Ok(command) => encode_command_response(runtime, &command, diag),
            Err(error) => {
                writeln!(diag, "axiom-stdio: command envelope rejected")?;
                let response = CommandResponse::from_error(error);
                encode_or_protocol(&response.to_value(), diag)
            }
        },
        Err(error) => {
            writeln!(diag, "axiom-stdio: {}", error.message())?;
            Ok(protocol_error_json(&error))
        }
    }
}

fn encode_command_response<D: Write>(
    runtime: &Runtime,
    command: &Command,
    diag: &mut D,
) -> std::io::Result<String> {
    let response = execute(runtime, command);
    encode_or_protocol(&response.to_value(), diag)
}

fn encode_or_protocol<D: Write>(value: &Value, diag: &mut D) -> std::io::Result<String> {
    match encode_value(value) {
        Ok(json) => Ok(json),
        Err(error) => {
            writeln!(diag, "axiom-stdio: {}", error.message())?;
            Ok(protocol_error_json(&error))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    use axiom_rs::runtime::Runtime;

    fn collect(input: &[u8]) -> (String, String) {
        let runtime = Runtime::new();
        let mut output = Vec::new();
        let mut diag = Vec::new();
        serve(&runtime, Cursor::new(input), &mut output, &mut diag).unwrap();
        (
            String::from_utf8(output).unwrap(),
            String::from_utf8(diag).unwrap(),
        )
    }

    #[test]
    fn oversized_line_then_next_request() {
        let mut input = " ".repeat(MAX_FRAME_BYTES + 1);
        input.push('\n');
        input.push_str("{\"v\":{\"$i\":\"1\"},\"cmd\":\"list\"}\n");
        let (out, diag) = collect(input.as_bytes());
        let mut lines = out.lines();
        let first = decode_value(lines.next().unwrap()).unwrap();
        assert_eq!(
            first
                .as_object()
                .unwrap()
                .get("error")
                .unwrap()
                .as_object()
                .unwrap()
                .get("kind")
                .and_then(Value::as_str),
            Some("limit_exceeded")
        );
        let second = decode_value(lines.next().unwrap()).unwrap();
        assert_eq!(
            second.as_object().unwrap().get("ok"),
            Some(&Value::bool(true))
        );
        assert!(diag.contains("MAX_FRAME_BYTES"));
    }

    #[test]
    fn exact_limit_without_newline_at_eof_is_accepted() {
        let line = "a".repeat(MAX_FRAME_BYTES);
        let (out, _) = collect(line.as_bytes());
        let value = decode_value(out.trim()).unwrap();
        assert_eq!(
            value
                .as_object()
                .unwrap()
                .get("error")
                .unwrap()
                .as_object()
                .unwrap()
                .get("kind")
                .and_then(Value::as_str),
            Some("malformed_json")
        );
    }

    #[test]
    fn overlong_input_without_newline_does_not_block_eof() {
        let input = "x".repeat(MAX_FRAME_BYTES + 8);
        let (out, _) = collect(input.as_bytes());
        assert_eq!(out.lines().count(), 1);
        let value = decode_value(out.trim()).unwrap();
        assert_eq!(
            value
                .as_object()
                .unwrap()
                .get("error")
                .unwrap()
                .as_object()
                .unwrap()
                .get("kind")
                .and_then(Value::as_str),
            Some("limit_exceeded")
        );
    }

    #[test]
    fn blank_lines_and_empty_eof_are_silent() {
        let (out, diag) = collect(b"\n  \n");
        assert!(out.is_empty());
        assert!(diag.is_empty());
    }
}
