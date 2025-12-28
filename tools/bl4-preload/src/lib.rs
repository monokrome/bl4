//! LD_PRELOAD library for intercepting file I/O
//!
//! Captures file reads/writes to detect NCS file access and extraction.
//!
//! Usage:
//!   LD_PRELOAD=/path/to/libbl4_preload.so ./ncs_extractor
//!
//! Environment variables:
//!   BL4_PRELOAD_LOG=<path>     - Log file path (default: /tmp/bl4_preload.log)
//!   BL4_PRELOAD_CAPTURE=<dir>  - Directory to save captured file writes
//!   BL4_PRELOAD_FILTER=<pat>   - Only capture files matching pattern (e.g., "*.json,*.ncs")
//!   BL4_PRELOAD_STACKS=1       - Log full stack traces

use backtrace::Backtrace;
use libc::{c_char, c_int, c_void, mode_t, off_t, size_t, ssize_t, O_CREAT, O_WRONLY};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::ffi::CStr;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use std::sync::atomic::{AtomicU64, Ordering};

// Per-thread re-entrancy guard using thread ID
static HOOK_THREAD: AtomicU64 = AtomicU64::new(0);

// RAII guard for hook re-entrancy
struct HookGuard;

impl HookGuard {
    fn try_enter() -> Option<Self> {
        let tid = unsafe { libc::pthread_self() } as u64;
        match HOOK_THREAD.compare_exchange(0, tid, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => Some(Self),                // We claimed it
            Err(owner) if owner == tid => None, // Re-entry, skip
            Err(_) => Some(Self),               // Another thread, proceed
        }
    }
}

impl Drop for HookGuard {
    fn drop(&mut self) {
        let tid = unsafe { libc::pthread_self() } as u64;
        let _ = HOOK_THREAD.compare_exchange(tid, 0, Ordering::SeqCst, Ordering::SeqCst);
    }
}

// Legacy functions for partially updated code
fn enter_hook() -> bool {
    HookGuard::try_enter().map(std::mem::forget).is_some()
}

fn exit_hook() {
    let tid = unsafe { libc::pthread_self() } as u64;
    let _ = HOOK_THREAD.compare_exchange(tid, 0, Ordering::SeqCst, Ordering::SeqCst);
}

// Original function pointers
static REAL_OPEN: Lazy<unsafe extern "C" fn(*const c_char, c_int, mode_t) -> c_int> =
    Lazy::new(|| unsafe {
        let ptr = libc::dlsym(libc::RTLD_NEXT, c"open".as_ptr());
        std::mem::transmute(ptr)
    });

static REAL_OPENAT: Lazy<unsafe extern "C" fn(c_int, *const c_char, c_int, mode_t) -> c_int> =
    Lazy::new(|| unsafe {
        let ptr = libc::dlsym(libc::RTLD_NEXT, c"openat".as_ptr());
        std::mem::transmute(ptr)
    });

static REAL_WRITE: Lazy<unsafe extern "C" fn(c_int, *const c_void, size_t) -> ssize_t> =
    Lazy::new(|| unsafe {
        let ptr = libc::dlsym(libc::RTLD_NEXT, c"write".as_ptr());
        std::mem::transmute(ptr)
    });

static REAL_READ: Lazy<unsafe extern "C" fn(c_int, *mut c_void, size_t) -> ssize_t> =
    Lazy::new(|| unsafe {
        let ptr = libc::dlsym(libc::RTLD_NEXT, c"read".as_ptr());
        std::mem::transmute(ptr)
    });

static REAL_PREAD: Lazy<unsafe extern "C" fn(c_int, *mut c_void, size_t, off_t) -> ssize_t> =
    Lazy::new(|| unsafe {
        let ptr = libc::dlsym(libc::RTLD_NEXT, c"pread".as_ptr());
        std::mem::transmute(ptr)
    });

static REAL_PREAD64: Lazy<unsafe extern "C" fn(c_int, *mut c_void, size_t, i64) -> ssize_t> =
    Lazy::new(|| unsafe {
        let ptr = libc::dlsym(libc::RTLD_NEXT, c"pread64".as_ptr());
        std::mem::transmute(ptr)
    });

static REAL_CLOSE: Lazy<unsafe extern "C" fn(c_int) -> c_int> = Lazy::new(|| unsafe {
    let ptr = libc::dlsym(libc::RTLD_NEXT, c"close".as_ptr());
    std::mem::transmute(ptr)
});

static REAL_LSEEK: Lazy<unsafe extern "C" fn(c_int, off_t, c_int) -> off_t> =
    Lazy::new(|| unsafe {
        let ptr = libc::dlsym(libc::RTLD_NEXT, c"lseek".as_ptr());
        std::mem::transmute(ptr)
    });

static REAL_MMAP: Lazy<
    unsafe extern "C" fn(*mut c_void, size_t, c_int, c_int, c_int, off_t) -> *mut c_void,
> = Lazy::new(|| unsafe {
    let ptr = libc::dlsym(libc::RTLD_NEXT, c"mmap".as_ptr());
    std::mem::transmute(ptr)
});

static REAL_MMAP64: Lazy<
    unsafe extern "C" fn(*mut c_void, size_t, c_int, c_int, c_int, i64) -> *mut c_void,
> = Lazy::new(|| unsafe {
    let ptr = libc::dlsym(libc::RTLD_NEXT, c"mmap64".as_ptr());
    std::mem::transmute(ptr)
});

// FILE* based I/O (stdio)
static REAL_FOPEN: Lazy<unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_void> =
    Lazy::new(|| unsafe {
        let ptr = libc::dlsym(libc::RTLD_NEXT, c"fopen".as_ptr());
        std::mem::transmute(ptr)
    });

static REAL_FREAD: Lazy<unsafe extern "C" fn(*mut c_void, size_t, size_t, *mut c_void) -> size_t> =
    Lazy::new(|| unsafe {
        let ptr = libc::dlsym(libc::RTLD_NEXT, c"fread".as_ptr());
        std::mem::transmute(ptr)
    });

static REAL_FWRITE: Lazy<
    unsafe extern "C" fn(*const c_void, size_t, size_t, *mut c_void) -> size_t,
> = Lazy::new(|| unsafe {
    let ptr = libc::dlsym(libc::RTLD_NEXT, c"fwrite".as_ptr());
    std::mem::transmute(ptr)
});

static REAL_FCLOSE: Lazy<unsafe extern "C" fn(*mut c_void) -> c_int> = Lazy::new(|| unsafe {
    let ptr = libc::dlsym(libc::RTLD_NEXT, c"fclose".as_ptr());
    std::mem::transmute(ptr)
});

// Track FILE* -> paths
static FILE_PATHS: Lazy<Mutex<HashMap<usize, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));

// Track FILE* write buffers
static FILE_WRITE_BUFFERS: Lazy<Mutex<HashMap<usize, Vec<u8>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// Track open file descriptors -> paths
static FD_PATHS: Lazy<Mutex<HashMap<c_int, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));

// Track file write buffers for capture
static WRITE_BUFFERS: Lazy<Mutex<HashMap<c_int, Vec<u8>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// Log file
static LOG_FILE: Lazy<Mutex<Option<File>>> = Lazy::new(|| {
    let path =
        std::env::var("BL4_PRELOAD_LOG").unwrap_or_else(|_| "/tmp/bl4_preload.log".to_string());
    let file = OpenOptions::new().create(true).append(true).open(path).ok();
    Mutex::new(file)
});

fn log(msg: &str) {
    // Lock-free append - may interleave but won't deadlock
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(std::env::var("BL4_PRELOAD_LOG").unwrap_or_else(|_| "/tmp/bl4_preload.log".into()))
    {
        let _ = writeln!(file, "{}", msg);
    }
}

fn should_log_stacks() -> bool {
    std::env::var("BL4_PRELOAD_STACKS")
        .map(|v| v == "1")
        .unwrap_or(false)
}

fn get_caller_addresses() -> Vec<usize> {
    // Only capture if explicitly enabled - backtrace is slow
    if std::env::var("BL4_PRELOAD_CALLERS")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        let bt = Backtrace::new_unresolved();
        bt.frames()
            .iter()
            .skip(3)
            .take(8)
            .map(|f| f.ip() as usize)
            .collect()
    } else {
        Vec::new()
    }
}

fn log_with_caller(msg: &str, callers: &[usize]) {
    // Use try_lock to avoid blocking/deadlock
    if let Ok(mut guard) = LOG_FILE.try_lock() {
        if let Some(ref mut file) = *guard {
            if callers.is_empty() {
                let _ = writeln!(file, "{}", msg);
            } else {
                let caller_str: String = callers
                    .iter()
                    .map(|a| format!("{:#x}", a))
                    .collect::<Vec<_>>()
                    .join(" <- ");
                let _ = writeln!(file, "{} | caller: {}", msg, caller_str);
            }

            if should_log_stacks() {
                let bt = Backtrace::new();
                let _ = writeln!(file, "{:?}", bt);
                let _ = writeln!(file, "---");
            }
        }
    }
}

fn get_capture_dir() -> Option<PathBuf> {
    std::env::var("BL4_PRELOAD_CAPTURE").ok().map(PathBuf::from)
}

fn get_filter_patterns() -> Vec<String> {
    std::env::var("BL4_PRELOAD_FILTER")
        .ok()
        .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
        .unwrap_or_default()
}

fn matches_filter(path: &str) -> bool {
    let patterns = get_filter_patterns();
    if patterns.is_empty() {
        return true;
    }
    patterns.iter().any(|pat| {
        if let Some(suffix) = pat.strip_prefix('*') {
            path.ends_with(suffix)
        } else if let Some(prefix) = pat.strip_suffix('*') {
            path.starts_with(prefix)
        } else {
            path.contains(pat)
        }
    })
}

fn is_interesting_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    // NCS-related keywords
    lower.contains("ncs")
        || lower.contains("inventory")
        || lower.contains("serial")
        || lower.contains("part")
        || lower.contains("item")
        || lower.contains("weapon")
        || lower.contains("balance")
        // Unreal Engine files (where NCS/uassets live)
        || lower.ends_with(".pak")
        || lower.ends_with(".utoc")
        || lower.ends_with(".ucas")
        || lower.ends_with(".uasset")
        || lower.ends_with(".uexp")
        || lower.ends_with(".umap")
        // Config/data files
        || lower.ends_with(".json")
        || lower.ends_with(".dat")
        || lower.ends_with(".ini")
        // Borderlands-specific paths
        || lower.contains("borderlands")
        || lower.contains("oakgame")
}

/// Hook for open()
///
/// # Safety
/// Caller must provide a valid null-terminated C string for `pathname`.
#[no_mangle]
pub unsafe extern "C" fn open(pathname: *const c_char, flags: c_int, mode: mode_t) -> c_int {
    let fd = REAL_OPEN(pathname, flags, mode);

    // Skip if re-entering (our log calls open)
    if !enter_hook() {
        return fd;
    }

    if fd >= 0 && !pathname.is_null() {
        if let Ok(path) = CStr::from_ptr(pathname).to_str() {
            // Track fd -> path mapping
            if let Ok(mut guard) = FD_PATHS.try_lock() {
                guard.insert(fd, path.to_string());
            }

            // Log interesting opens
            if is_interesting_path(path) {
                let mode_str = if flags & O_WRONLY != 0 || flags & O_CREAT != 0 {
                    "WRITE"
                } else {
                    "READ"
                };
                let callers = get_caller_addresses();
                log_with_caller(
                    &format!("[OPEN] {} {} (fd={})", mode_str, path, fd),
                    &callers,
                );

                // Initialize write buffer for files we're writing
                if flags & O_WRONLY != 0 || flags & O_CREAT != 0 {
                    if let Ok(mut guard) = WRITE_BUFFERS.try_lock() {
                        guard.insert(fd, Vec::new());
                    }
                }
            }
        }
    }

    exit_hook();
    fd
}

/// Hook for openat()
///
/// # Safety
/// Caller must provide a valid null-terminated C string for `pathname`.
#[no_mangle]
pub unsafe extern "C" fn openat(
    dirfd: c_int,
    pathname: *const c_char,
    flags: c_int,
    mode: mode_t,
) -> c_int {
    let fd = REAL_OPENAT(dirfd, pathname, flags, mode);

    if !enter_hook() {
        return fd;
    }

    if fd >= 0 && !pathname.is_null() {
        if let Ok(path) = CStr::from_ptr(pathname).to_str() {
            if let Ok(mut guard) = FD_PATHS.try_lock() {
                guard.insert(fd, path.to_string());
            }

            if is_interesting_path(path) {
                let mode_str = if flags & O_WRONLY != 0 || flags & O_CREAT != 0 {
                    "WRITE"
                } else {
                    "READ"
                };
                let callers = get_caller_addresses();
                log_with_caller(
                    &format!(
                        "[OPENAT] {} {} (dirfd={}, fd={})",
                        mode_str, path, dirfd, fd
                    ),
                    &callers,
                );

                if flags & O_WRONLY != 0 || flags & O_CREAT != 0 {
                    if let Ok(mut guard) = WRITE_BUFFERS.try_lock() {
                        guard.insert(fd, Vec::new());
                    }
                }
            }
        }
    }

    exit_hook();
    fd
}

/// Hook for write()
///
/// # Safety
/// Caller must provide a valid buffer of at least `count` bytes.
#[no_mangle]
pub unsafe extern "C" fn write(fd: c_int, buf: *const c_void, count: size_t) -> ssize_t {
    let result = REAL_WRITE(fd, buf, count);

    if !enter_hook() {
        return result;
    }

    // Capture write data if we're tracking this fd
    if result > 0 && !buf.is_null() {
        let should_capture = {
            let guard = WRITE_BUFFERS.try_lock().ok();
            guard.map(|g| g.contains_key(&fd)).unwrap_or(false)
        };

        if should_capture {
            let bytes = std::slice::from_raw_parts(buf as *const u8, result as usize);
            if let Ok(mut guard) = WRITE_BUFFERS.try_lock() {
                if let Some(buffer) = guard.get_mut(&fd) {
                    buffer.extend_from_slice(bytes);
                }
            }
        }
    }

    exit_hook();
    result
}

/// Hook for read()
///
/// # Safety
/// Caller must provide a valid writable buffer of at least `count` bytes.
#[no_mangle]
pub unsafe extern "C" fn read(fd: c_int, buf: *mut c_void, count: size_t) -> ssize_t {
    let result = REAL_READ(fd, buf, count);

    if !enter_hook() {
        return result;
    }

    // Log reads from interesting files
    if result > 0 {
        let path = FD_PATHS.try_lock().ok().and_then(|g| g.get(&fd).cloned());

        if let Some(path) = path {
            if is_interesting_path(&path) {
                let callers = get_caller_addresses();
                log_with_caller(&format!("[READ] {} bytes from {}", result, path), &callers);
            }
        }
    }

    exit_hook();
    result
}

/// Hook for pread() - positioned read (common for PAK files)
///
/// # Safety
/// Caller must provide a valid writable buffer of at least `count` bytes.
#[no_mangle]
pub unsafe extern "C" fn pread(
    fd: c_int,
    buf: *mut c_void,
    count: size_t,
    offset: off_t,
) -> ssize_t {
    let result = REAL_PREAD(fd, buf, count, offset);

    let Some(_guard) = HookGuard::try_enter() else {
        return result;
    };

    if result > 0 {
        let path = FD_PATHS.try_lock().ok().and_then(|g| g.get(&fd).cloned());

        if let Some(path) = path {
            if is_interesting_path(&path) {
                log(&format!(
                    "[PREAD] {} bytes at offset {} from {}",
                    result, offset, path
                ));
            }
        }
    }

    result
}

/// Hook for pread64() - 64-bit positioned read
///
/// # Safety
/// Caller must provide a valid writable buffer of at least `count` bytes.
#[no_mangle]
pub unsafe extern "C" fn pread64(
    fd: c_int,
    buf: *mut c_void,
    count: size_t,
    offset: i64,
) -> ssize_t {
    let result = REAL_PREAD64(fd, buf, count, offset);

    let Some(_guard) = HookGuard::try_enter() else {
        return result;
    };

    if result > 0 {
        let path = FD_PATHS.try_lock().ok().and_then(|g| g.get(&fd).cloned());

        if let Some(path) = path {
            if is_interesting_path(&path) {
                log(&format!(
                    "[PREAD64] {} bytes at offset {} from {}",
                    result, offset, path
                ));
            }
        }
    }

    result
}

/// Hook for close()
///
/// # Safety
/// Caller must provide a valid file descriptor.
#[no_mangle]
pub unsafe extern "C" fn close(fd: c_int) -> c_int {
    // Always call real close first
    let result = REAL_CLOSE(fd);

    if !enter_hook() {
        return result;
    }

    // Save captured data after closing
    let captured = WRITE_BUFFERS
        .try_lock()
        .ok()
        .and_then(|mut g| g.remove(&fd));
    let path = FD_PATHS.try_lock().ok().and_then(|mut g| g.remove(&fd));

    if let (Some(data), Some(path)) = (captured, path.as_ref()) {
        if !data.is_empty() && matches_filter(path) {
            log(&format!(
                "[CLOSE] Captured {} bytes written to {}",
                data.len(),
                path
            ));

            // Save to capture directory if configured
            if let Some(capture_dir) = get_capture_dir() {
                let _ = fs::create_dir_all(&capture_dir);
                let filename = PathBuf::from(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| format!("captured_{}", fd));
                let capture_path = capture_dir.join(&filename);

                if let Ok(mut file) = File::create(&capture_path) {
                    let _ = file.write_all(&data);
                    log(&format!("[SAVED] {} -> {:?}", path, capture_path));
                }
            }
        }
    }

    exit_hook();
    result
}

/// Hook for lseek() - just pass through, but log for interesting files
///
/// # Safety
/// Caller must provide a valid file descriptor.
#[no_mangle]
pub unsafe extern "C" fn lseek(fd: c_int, offset: off_t, whence: c_int) -> off_t {
    REAL_LSEEK(fd, offset, whence)
}

/// Hook for mmap() - memory-mapped file access (common for PAK files)
///
/// # Safety
/// Caller must follow mmap(2) contract: valid fd (if not MAP_ANONYMOUS),
/// valid addr hint, and valid prot/flags combination.
#[no_mangle]
pub unsafe extern "C" fn mmap(
    addr: *mut c_void,
    length: size_t,
    prot: c_int,
    flags: c_int,
    fd: c_int,
    offset: off_t,
) -> *mut c_void {
    let result = REAL_MMAP(addr, length, prot, flags, fd, offset);

    let Some(_guard) = HookGuard::try_enter() else {
        return result;
    };

    if !result.is_null() && result != libc::MAP_FAILED && fd >= 0 {
        let path = FD_PATHS.try_lock().ok().and_then(|g| g.get(&fd).cloned());

        if let Some(path) = path {
            if is_interesting_path(&path) {
                let callers = get_caller_addresses();
                log_with_caller(
                    &format!(
                        "[MMAP] {} bytes at offset {} from {} (fd={}, addr={:?})",
                        length, offset, path, fd, result
                    ),
                    &callers,
                );
            }
        }
    }

    result
}

/// Hook for mmap64() - 64-bit offset version
///
/// # Safety
/// Caller must follow mmap(2) contract: valid fd (if not MAP_ANONYMOUS),
/// valid addr hint, and valid prot/flags combination.
#[no_mangle]
pub unsafe extern "C" fn mmap64(
    addr: *mut c_void,
    length: size_t,
    prot: c_int,
    flags: c_int,
    fd: c_int,
    offset: i64,
) -> *mut c_void {
    let result = REAL_MMAP64(addr, length, prot, flags, fd, offset);

    let Some(_guard) = HookGuard::try_enter() else {
        return result;
    };

    if !result.is_null() && result != libc::MAP_FAILED && fd >= 0 {
        let path = FD_PATHS.try_lock().ok().and_then(|g| g.get(&fd).cloned());

        if let Some(path) = path {
            if is_interesting_path(&path) {
                let callers = get_caller_addresses();
                log_with_caller(
                    &format!(
                        "[MMAP64] {} bytes at offset {} from {} (fd={}, addr={:?})",
                        length, offset, path, fd, result
                    ),
                    &callers,
                );
            }
        }
    }

    result
}

/// Hook for fopen() - stdio file open
///
/// # Safety
/// Caller must provide valid null-terminated C strings for `pathname` and `mode`.
#[no_mangle]
pub unsafe extern "C" fn fopen(pathname: *const c_char, mode: *const c_char) -> *mut c_void {
    let fp = REAL_FOPEN(pathname, mode);

    let Some(_guard) = HookGuard::try_enter() else {
        return fp;
    };

    if !fp.is_null() && !pathname.is_null() {
        if let Ok(path) = CStr::from_ptr(pathname).to_str() {
            let fp_key = fp as usize;

            // Track FILE* -> path
            if let Ok(mut guard) = FILE_PATHS.try_lock() {
                guard.insert(fp_key, path.to_string());
            }

            // Log interesting opens
            if is_interesting_path(path) {
                let mode_str = if !mode.is_null() {
                    CStr::from_ptr(mode).to_str().unwrap_or("?")
                } else {
                    "?"
                };
                let callers = get_caller_addresses();
                log_with_caller(
                    &format!("[FOPEN] {} mode={} (fp={:#x})", path, mode_str, fp_key),
                    &callers,
                );

                // Track writes for capture
                if mode_str.contains('w') || mode_str.contains('a') {
                    if let Ok(mut guard) = FILE_WRITE_BUFFERS.try_lock() {
                        guard.insert(fp_key, Vec::new());
                    }
                }
            }
        }
    }

    fp
}

/// Hook for fread() - stdio file read
///
/// # Safety
/// Caller must provide a valid writable buffer of at least `size * nmemb` bytes
/// and a valid FILE stream pointer.
#[no_mangle]
pub unsafe extern "C" fn fread(
    ptr: *mut c_void,
    size: size_t,
    nmemb: size_t,
    stream: *mut c_void,
) -> size_t {
    let result = REAL_FREAD(ptr, size, nmemb, stream);

    let Some(_guard) = HookGuard::try_enter() else {
        return result;
    };

    if result > 0 {
        let fp_key = stream as usize;
        let path = FILE_PATHS
            .try_lock()
            .ok()
            .and_then(|g| g.get(&fp_key).cloned());

        if let Some(path) = path {
            if is_interesting_path(&path) {
                let bytes_read = result * size;
                let callers = get_caller_addresses();
                log_with_caller(
                    &format!("[FREAD] {} bytes from {}", bytes_read, path),
                    &callers,
                );
            }
        }
    }

    result
}

/// Hook for fwrite() - stdio file write
///
/// # Safety
/// Caller must provide a valid buffer of at least `size * nmemb` bytes
/// and a valid FILE stream pointer.
#[no_mangle]
pub unsafe extern "C" fn fwrite(
    ptr: *const c_void,
    size: size_t,
    nmemb: size_t,
    stream: *mut c_void,
) -> size_t {
    let result = REAL_FWRITE(ptr, size, nmemb, stream);

    let Some(_guard) = HookGuard::try_enter() else {
        return result;
    };

    if result > 0 && !ptr.is_null() {
        let fp_key = stream as usize;
        let bytes_written = result * size;

        // Capture write data if we're tracking this FILE*
        let should_capture = FILE_WRITE_BUFFERS
            .try_lock()
            .ok()
            .map(|g| g.contains_key(&fp_key))
            .unwrap_or(false);

        if should_capture {
            let bytes = std::slice::from_raw_parts(ptr as *const u8, bytes_written);
            if let Ok(mut guard) = FILE_WRITE_BUFFERS.try_lock() {
                if let Some(buffer) = guard.get_mut(&fp_key) {
                    buffer.extend_from_slice(bytes);
                }
            }
        }
    }

    result
}

/// Hook for fclose() - stdio file close
///
/// # Safety
/// Caller must provide a valid FILE stream pointer.
#[no_mangle]
pub unsafe extern "C" fn fclose(stream: *mut c_void) -> c_int {
    let result = REAL_FCLOSE(stream);

    let Some(_guard) = HookGuard::try_enter() else {
        return result;
    };

    let fp_key = stream as usize;

    // Save captured data after closing
    let captured = FILE_WRITE_BUFFERS
        .try_lock()
        .ok()
        .and_then(|mut g| g.remove(&fp_key));
    let path = FILE_PATHS
        .try_lock()
        .ok()
        .and_then(|mut g| g.remove(&fp_key));

    if let (Some(data), Some(path)) = (captured, path.as_ref()) {
        if !data.is_empty() && matches_filter(path) {
            log(&format!(
                "[FCLOSE] Captured {} bytes written to {}",
                data.len(),
                path
            ));

            // Save to capture directory if configured
            if let Some(capture_dir) = get_capture_dir() {
                let _ = fs::create_dir_all(&capture_dir);
                let filename = PathBuf::from(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| format!("captured_{}", fp_key));
                let capture_path = capture_dir.join(&filename);

                if let Ok(mut file) = File::create(&capture_path) {
                    let _ = file.write_all(&data);
                    log(&format!("[SAVED] {} -> {:?}", path, capture_path));
                }
            }
        }
    }

    result
}

#[ctor::ctor]
fn init() {
    let log_path =
        std::env::var("BL4_PRELOAD_LOG").unwrap_or_else(|_| "/tmp/bl4_preload.log".to_string());

    if let Ok(mut file) = File::create(&log_path) {
        let _ = writeln!(file, "=== bl4-preload (file I/O capture) ===");
        let _ = writeln!(file, "PID: {}", std::process::id());
        let _ = writeln!(file, "Log: {}", log_path);
        if let Some(dir) = get_capture_dir() {
            let _ = writeln!(file, "Capture dir: {:?}", dir);
        }
        let patterns = get_filter_patterns();
        if !patterns.is_empty() {
            let _ = writeln!(file, "Filter: {:?}", patterns);
        }
        let _ = writeln!(file, "======================================");
    }

    // Force init of function pointers
    let _ = *REAL_OPEN;
    let _ = *REAL_OPENAT;
    let _ = *REAL_WRITE;
    let _ = *REAL_READ;
    let _ = *REAL_PREAD;
    let _ = *REAL_PREAD64;
    let _ = *REAL_CLOSE;
    let _ = *REAL_LSEEK;
    let _ = *REAL_MMAP;
    let _ = *REAL_MMAP64;
    let _ = *REAL_FOPEN;
    let _ = *REAL_FREAD;
    let _ = *REAL_FWRITE;
    let _ = *REAL_FCLOSE;
}
