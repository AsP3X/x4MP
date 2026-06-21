// Human: Windows named-pipe server for SirNuke Pipe API (X4 is client, bridge is server).
// Agent: READS \\.\pipe\<name> messages; SENDS UTF-8 lines to bridge via tokio mpsc channel.
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use tokio::sync::mpsc::UnboundedSender;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    FlushFileBuffers, ReadFile, PIPE_ACCESS_DUPLEX,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_MESSAGE,
    PIPE_TYPE_MESSAGE, PIPE_WAIT,
};

// Human: Encode a Rust str as a null-terminated UTF-16 string for Win32 APIs.
// Agent: RETURNS Vec<u16> suitable for PCWSTR.
fn to_wide(value: &str) -> Vec<u16> {
    OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

// Human: Blocking loop hosting one SirNuke-compatible message-mode named pipe.
// Agent: READS X4 Pipe.write payloads; EMITS each message on tx until channel closes.
pub fn serve_named_pipe(pipe_name: &str, tx: UnboundedSender<String>) -> Result<(), String> {
    let path = format!(r"\\.\pipe\{pipe_name}");
    let wide = to_wide(&path);

    // Human: Match SirNuke Pipe_Server buffer sizes and message mode.
    // Agent: CALLS CreateNamedPipeW once; reconnects with ConnectNamedPipe after X4 reload.
    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(wide.as_ptr()),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
            1,
            65536,
            65536,
            300,
            None,
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        return Err(format!("CreateNamedPipeW failed for {path}"));
    }

    tracing::info!(pipe = pipe_name, path = %path, "bridge named-pipe server listening");

    loop {
        unsafe {
            let connect_result = ConnectNamedPipe(handle, None);
            if let Err(err) = connect_result {
                // ERROR_PIPE_CONNECTED (535) means client already connected — continue.
                if err.code().0 as u32 != windows::Win32::Foundation::ERROR_PIPE_CONNECTED.0 {
                    return Err(format!("ConnectNamedPipe({path}): {err}"));
                }
            }
        }

        tracing::info!(pipe = pipe_name, "X4 connected to bridge named pipe");

        loop {
            let mut buffer = vec![0u8; 65536];
            let mut bytes_read = 0u32;
            let read_ok = unsafe {
                ReadFile(
                    handle,
                    Some(buffer.as_mut_slice()),
                    Some(&mut bytes_read),
                    None,
                )
            };

            match read_ok {
                Ok(()) => {
                    if bytes_read == 0 {
                        continue;
                    }
                    let message =
                        String::from_utf8_lossy(&buffer[..bytes_read as usize]).into_owned();
                    if message == "garbage_collected" {
                        tracing::info!(pipe = pipe_name, "pipe client garbage collected");
                        break;
                    }
                    if tx.send(message).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    tracing::warn!(pipe = pipe_name, error = %err, "named pipe read failed");
                    break;
                }
            }
        }

        if tx.is_closed() {
            break;
        }

        unsafe {
            let _ = FlushFileBuffers(handle);
            let _ = DisconnectNamedPipe(handle);
        }
        tracing::info!(pipe = pipe_name, "X4 disconnected; waiting for reconnect");
    }

    unsafe {
        let _ = CloseHandle(handle);
    }
    Ok(())
}
