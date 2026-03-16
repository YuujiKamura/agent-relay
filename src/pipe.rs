use crate::error::{AgentCtlError, Result};
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    CreateFileA, FlushFileBuffers, ReadFile, WriteFile, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ,
    FILE_GENERIC_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Pipes::WaitNamedPipeA;
use windows::core::PCSTR;

const MAX_RETRIES: u32 = 3;
const RETRY_BACKOFF_MS: u64 = 2000;
const WAIT_PIPE_TIMEOUT_MS: u32 = 5000;
const READ_BUFFER_SIZE: usize = 65536;

/// Send a message to the named pipe and return the response.
/// Retries on transient failures (pipe busy, file not found).
pub fn send_pipe_message(pipe_path: &str, message: &str) -> Result<String> {
    let mut last_err = AgentCtlError::PipeConnect("no attempts made".into());

    for attempt in 0..MAX_RETRIES {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(RETRY_BACKOFF_MS));
        }

        match try_send(pipe_path, message) {
            Ok(response) => return Ok(response),
            Err(e) => {
                last_err = e;
                // Continue retrying on transient errors
            }
        }
    }

    Err(last_err)
}

fn try_send(pipe_path: &str, message: &str) -> Result<String> {
    // Wait for the pipe server to become available
    let pipe_path_cstr = format!("{}\0", pipe_path);
    let wait_result =
        unsafe { WaitNamedPipeA(PCSTR(pipe_path_cstr.as_ptr()), WAIT_PIPE_TIMEOUT_MS) };

    if let Err(_) = wait_result {
        // WaitNamedPipe failed — pipe may not exist yet or server is down
        let err = std::io::Error::last_os_error();
        return Err(AgentCtlError::PipeConnect(format!(
            "WaitNamedPipe failed: {}",
            err
        )));
    }

    // Open the pipe as a file
    let handle = unsafe {
        CreateFileA(
            PCSTR(pipe_path_cstr.as_ptr()),
            (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
            windows::Win32::Storage::FileSystem::FILE_SHARE_NONE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            HANDLE::default(),
        )
    }
    .map_err(|e| AgentCtlError::PipeConnect(format!("CreateFile failed: {}", e)))?;

    if handle == INVALID_HANDLE_VALUE {
        return Err(AgentCtlError::PipeConnect(
            "CreateFile returned INVALID_HANDLE_VALUE".into(),
        ));
    }

    // Ensure we close the handle on all exit paths
    let _guard = HandleGuard(handle);

    // Write message
    let msg_bytes = message.as_bytes();
    let mut written = 0u32;
    unsafe {
        WriteFile(handle, Some(msg_bytes), Some(&mut written), None)
            .map_err(|e| AgentCtlError::PipeIo(format!("WriteFile failed: {}", e)))?;
        FlushFileBuffers(handle)
            .map_err(|e| AgentCtlError::PipeIo(format!("FlushFileBuffers failed: {}", e)))?;
    }

    // Read response
    let mut buffer = vec![0u8; READ_BUFFER_SIZE];
    let mut read = 0u32;
    unsafe {
        ReadFile(handle, Some(&mut buffer), Some(&mut read), None)
            .map_err(|e| AgentCtlError::PipeIo(format!("ReadFile failed: {}", e)))?;
    }

    if read == 0 {
        return Err(AgentCtlError::PipeIo("empty response".into()));
    }

    let response = String::from_utf8_lossy(&buffer[..read as usize]).to_string();
    Ok(response)
}

/// RAII guard for Win32 HANDLE.
struct HandleGuard(HANDLE);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
