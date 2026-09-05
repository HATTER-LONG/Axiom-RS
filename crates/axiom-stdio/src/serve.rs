//! NDJSON request/response loop.

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
    input: R,
    mut output: W,
    mut diag: D,
) -> std::io::Result<()>
where
    R: BufRead,
    W: Write,
    D: Write,
{
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = handle_line(runtime, &line, &mut diag)?;
        output.write_all(response.as_bytes())?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}

fn handle_line<D: Write>(runtime: &Runtime, line: &str, diag: &mut D) -> std::io::Result<String> {
    if line.len() > MAX_FRAME_BYTES {
        let error = AdapterError::limit_exceeded();
        writeln!(diag, "axiom-stdio: {}", error.message())?;
        return Ok(protocol_error_json(&error));
    }
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
