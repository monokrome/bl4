//! Oodle decompression abstraction
//!
//! Provides a trait-based interface for Oodle decompression with multiple backends:
//! - `OozextractBackend`: Open-source implementation (default, ~97.6% compatibility)
//! - `NativeBackend`: Uses the official Oodle DLL via FFI (Windows only)
//! - `ExecBackend`: Executes an external command for decompression (cross-platform)

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;

use crate::{Error, Result};

/// Trait for Oodle block decompression
pub trait OodleDecompressor: Send + Sync {
    /// Decompress a single Oodle-compressed block
    ///
    /// # Arguments
    /// * `compressed` - The compressed block data
    /// * `decompressed_size` - Expected size after decompression
    ///
    /// # Returns
    /// The decompressed data, or an error if decompression fails
    fn decompress_block(&self, compressed: &[u8], decompressed_size: usize) -> Result<Vec<u8>>;

    /// Get the backend name for diagnostics
    fn name(&self) -> &'static str;

    /// Check if this backend supports all Oodle compression variants
    ///
    /// The oozextract backend returns false as it doesn't support all variants.
    fn is_full_support(&self) -> bool {
        false
    }
}

/// Open-source Oodle decompressor using oozextract crate
///
/// This is the default backend. It supports most Oodle-compressed data but
/// may fail on certain compression parameters used by some games.
pub struct OozextractBackend {
    extractor: Mutex<oozextract::Extractor>,
}

impl OozextractBackend {
    pub fn new() -> Self {
        Self {
            extractor: Mutex::new(oozextract::Extractor::new()),
        }
    }
}

impl Default for OozextractBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for OozextractBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OozextractBackend").finish_non_exhaustive()
    }
}

impl OodleDecompressor for OozextractBackend {
    fn decompress_block(&self, compressed: &[u8], decompressed_size: usize) -> Result<Vec<u8>> {
        let mut output = vec![0u8; decompressed_size];
        let mut extractor = self.extractor.lock().unwrap();

        let actual = extractor
            .read_from_slice(compressed, &mut output)
            .map_err(|e| Error::Oodle(format!("oozextract: {:?}", e)))?;

        if actual != decompressed_size {
            return Err(Error::DecompressionSize {
                expected: decompressed_size,
                actual,
            });
        }

        Ok(output)
    }

    fn name(&self) -> &'static str {
        "oozextract"
    }

    fn is_full_support(&self) -> bool {
        false
    }
}

/// Native Oodle decompressor using the official DLL
///
/// Requires the Oodle DLL (e.g., oo2core_9_win64.dll) to be available.
/// This backend supports all Oodle compression variants.
#[cfg(target_os = "windows")]
pub struct NativeBackend {
    decompress_fn: OodleLzDecompress,
}

#[cfg(target_os = "windows")]
type OodleLzDecompress = unsafe extern "C" fn(
    comp_buf: *const u8,
    comp_len: isize,
    raw_buf: *mut u8,
    raw_len: isize,
    fuzz_safe: i32,
    check_crc: i32,
    verbosity: i32,
    dec_buf_base: *mut u8,
    dec_buf_size: isize,
    fp_callback: *mut std::ffi::c_void,
    callback_user_data: *mut std::ffi::c_void,
    decoder_memory: *mut u8,
    decoder_memory_size: isize,
    thread_phase: i32,
) -> isize;

#[cfg(target_os = "windows")]
impl NativeBackend {
    /// Load the native Oodle backend from a DLL path
    pub fn load<P: AsRef<Path>>(dll_path: P) -> Result<Self> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let path = dll_path.as_ref();
        let wide_path: Vec<u16> = OsStr::new(path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let handle = winapi::um::libloaderapi::LoadLibraryW(wide_path.as_ptr());
            if handle.is_null() {
                return Err(Error::Oodle(format!(
                    "Failed to load Oodle DLL: {}",
                    path.display()
                )));
            }

            let proc_name = b"OodleLZ_Decompress\0";
            let proc = winapi::um::libloaderapi::GetProcAddress(handle, proc_name.as_ptr() as *const i8);
            if proc.is_null() {
                return Err(Error::Oodle(
                    "Failed to find OodleLZ_Decompress in DLL".to_string(),
                ));
            }

            Ok(Self {
                decompress_fn: std::mem::transmute(proc),
            })
        }
    }
}

#[cfg(target_os = "windows")]
impl OodleDecompressor for NativeBackend {
    fn decompress_block(&self, compressed: &[u8], decompressed_size: usize) -> Result<Vec<u8>> {
        let mut output = vec![0u8; decompressed_size];

        let result = unsafe {
            (self.decompress_fn)(
                compressed.as_ptr(),
                compressed.len() as isize,
                output.as_mut_ptr(),
                decompressed_size as isize,
                1,  // fuzz_safe
                0,  // check_crc
                0,  // verbosity
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
                0,
            )
        };

        if result < 0 {
            return Err(Error::Oodle(format!(
                "OodleLZ_Decompress failed with code {}",
                result
            )));
        }

        if result as usize != decompressed_size {
            return Err(Error::DecompressionSize {
                expected: decompressed_size,
                actual: result as usize,
            });
        }

        Ok(output)
    }

    fn name(&self) -> &'static str {
        "native"
    }

    fn is_full_support(&self) -> bool {
        true
    }
}


/// External command-based Oodle decompressor
///
/// Executes an external command for each decompression operation.
/// The command receives compressed data via stdin and outputs decompressed data to stdout.
///
/// # Protocol
///
/// The command is invoked as:
/// ```text
/// <command> decompress <decompressed_size>
/// ```
///
/// - Compressed data is written to the command's stdin
/// - Decompressed data is read from the command's stdout
/// - Exit code 0 indicates success
///
/// # Example Commands
///
/// A simple wrapper script could be:
/// ```bash
/// #!/bin/bash
/// # oodle-decompress.sh - wrapper for Oodle decompression via Wine
/// wine /path/to/oodle_helper.exe "$@"
/// ```
pub struct ExecBackend {
    command: String,
}

impl ExecBackend {
    /// Create a new exec backend with the given command
    ///
    /// The command should be an executable that accepts the decompression protocol.
    pub fn new<S: Into<String>>(command: S) -> Self {
        Self {
            command: command.into(),
        }
    }
}

impl std::fmt::Debug for ExecBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecBackend")
            .field("command", &self.command)
            .finish()
    }
}

impl OodleDecompressor for ExecBackend {
    fn decompress_block(&self, compressed: &[u8], decompressed_size: usize) -> Result<Vec<u8>> {
        let parts: Vec<&str> = self.command.split_whitespace().collect();
        let (program, prefix_args) = parts
            .split_first()
            .ok_or_else(|| Error::Oodle("Empty exec command".into()))?;

        let mut child = Command::new(program)
            .args(prefix_args)
            .arg("decompress")
            .arg(decompressed_size.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Oodle(format!("Failed to spawn command '{}': {}", self.command, e)))?;

        // Write compressed data to stdin
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(compressed)
                .map_err(|e| Error::Oodle(format!("Failed to write to command stdin: {}", e)))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| Error::Oodle(format!("Failed to wait for command: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Oodle(format!(
                "Command '{}' failed with exit code {:?}: {}",
                self.command,
                output.status.code(),
                stderr.trim()
            )));
        }

        if output.stdout.len() != decompressed_size {
            return Err(Error::DecompressionSize {
                expected: decompressed_size,
                actual: output.stdout.len(),
            });
        }

        Ok(output.stdout)
    }

    fn name(&self) -> &'static str {
        "exec"
    }

    fn is_full_support(&self) -> bool {
        true
    }
}

/// Create the default decompressor backend
pub fn default_backend() -> Box<dyn OodleDecompressor> {
    Box::new(OozextractBackend::new())
}

/// Create a native backend from a DLL path (Windows only)
#[cfg(target_os = "windows")]
pub fn native_backend<P: AsRef<Path>>(dll_path: P) -> Result<Box<dyn OodleDecompressor>> {
    Ok(Box::new(NativeBackend::load(dll_path)?))
}

/// Native Oodle DLL loading is not available on this platform
#[cfg(not(target_os = "windows"))]
pub fn native_backend<P: AsRef<Path>>(_dll_path: P) -> Result<Box<dyn OodleDecompressor>> {
    Err(Error::Oodle(
        "Native Oodle DLL loading requires Windows. On Linux/macOS, use \
         --oodle-exec with a decompression helper (add --oodle-fifo for Wine)"
            .to_string(),
    ))
}

/// Create an exec backend with the given command
pub fn exec_backend<S: Into<String>>(command: S) -> Box<dyn OodleDecompressor> {
    Box::new(ExecBackend::new(command))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oozextract_backend_name() {
        let backend = OozextractBackend::new();
        assert_eq!(backend.name(), "oozextract");
        assert!(!backend.is_full_support());
    }

    #[test]
    fn test_default_backend() {
        let backend = default_backend();
        assert_eq!(backend.name(), "oozextract");
    }

    #[test]
    fn test_exec_backend_empty_command() {
        let backend = ExecBackend::new("");
        let result = backend.decompress_block(&[0u8; 4], 4);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Empty exec command"), "got: {}", err);
    }

    #[test]
    fn test_exec_backend_single_word_command() {
        let backend = ExecBackend::new("nonexistent_oodle_helper");
        let result = backend.decompress_block(&[0u8; 4], 4);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Failed to spawn"), "got: {}", err);
    }

    #[test]
    fn test_exec_backend_multi_word_command() {
        let backend = ExecBackend::new("nonexistent_wine nonexistent_helper.exe --dll foo.dll");
        let result = backend.decompress_block(&[0u8; 4], 4);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Failed to spawn"), "got: {}", err);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_native_backend_not_available_on_non_windows() {
        match native_backend("/path/to/oodle.dll") {
            Err(e) => {
                let msg = e.to_string();
                assert!(
                    msg.contains("--oodle-exec"),
                    "error should suggest --oodle-exec, got: {}",
                    msg
                );
            }
            Ok(_) => panic!("native_backend should return Err on non-Windows"),
        }
    }
}
