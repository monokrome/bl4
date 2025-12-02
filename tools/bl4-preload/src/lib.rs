//! LD_PRELOAD library for intercepting calls in Borderlands 4
//!
//! This library hooks various functions to trace and optionally modify
//! game behavior without triggering anti-debug detection.
//!
//! Usage:
//!   LD_PRELOAD=/path/to/libbl4_preload.so steam steam://rungameid/...
//!
//! Environment variables:
//!   BL4_PRELOAD_LOG=<path>  - Log file path (default: /tmp/bl4_preload.log)
//!   BL4_PRELOAD_STACKS=1    - Log full stack traces
//!   BL4_PRELOAD_ALL=1       - Log every call (not just every 1000th)
//!
//! Modification modes (set before launching game):
//!   BL4_RNG_BIAS=<mode>     - Bias all RNG values
//!     - "max"      : Always return maximum values
//!     - "high"     : Bias toward high values (75th percentile)
//!     - "low"      : Bias toward low values (25th percentile)
//!     - "min"      : Always return minimum values
//!     - unset      : Normal behavior (no modification)
//!
//! Note: This affects ALL randomness (drops, rarity, damage, AI, etc).
//! Test with max vs min to see which direction improves loot.
//!
//! Output is written to /tmp/bl4_preload.log by default

use backtrace::Backtrace;
use libc::{c_int, c_uint, c_void};
use once_cell::sync::Lazy;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// Get log file path from environment or use default
fn get_log_path() -> String {
    std::env::var("BL4_PRELOAD_LOG").unwrap_or_else(|_| "/tmp/bl4_preload.log".to_string())
}

// Log file handle
static LOG_FILE: Lazy<Mutex<Option<File>>> = Lazy::new(|| {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(get_log_path())
        .ok();
    Mutex::new(file)
});

// Original function pointers (resolved via dlsym)
static REAL_RAND: Lazy<unsafe extern "C" fn() -> c_int> = Lazy::new(|| unsafe {
    let ptr = libc::dlsym(libc::RTLD_NEXT, b"rand\0".as_ptr() as *const _);
    if ptr.is_null() {
        panic!("Failed to find real rand()");
    }
    std::mem::transmute(ptr)
});

static REAL_SRAND: Lazy<unsafe extern "C" fn(c_uint)> = Lazy::new(|| unsafe {
    let ptr = libc::dlsym(libc::RTLD_NEXT, b"srand\0".as_ptr() as *const _);
    if ptr.is_null() {
        panic!("Failed to find real srand()");
    }
    std::mem::transmute(ptr)
});

use libc::c_long;

static REAL_RANDOM: Lazy<Option<unsafe extern "C" fn() -> c_long>> = Lazy::new(|| unsafe {
    let ptr = libc::dlsym(libc::RTLD_NEXT, b"random\0".as_ptr() as *const _);
    if ptr.is_null() {
        None
    } else {
        Some(std::mem::transmute(ptr))
    }
});

// Configurable options via environment variables
fn should_log_stacks() -> bool {
    std::env::var("BL4_PRELOAD_STACKS").map(|v| v == "1").unwrap_or(false)
}

fn should_log_all() -> bool {
    std::env::var("BL4_PRELOAD_ALL").map(|v| v == "1").unwrap_or(false)
}

/// Drop bias mode - how to modify RNG values
#[derive(Clone, Copy, Debug, PartialEq)]
enum DropBias {
    None,   // No modification
    Max,    // Always maximum
    High,   // Bias toward high (75th percentile)
    Low,    // Bias toward low (25th percentile)
    Min,    // Always minimum
}

static RNG_BIAS: Lazy<DropBias> = Lazy::new(|| {
    match std::env::var("BL4_RNG_BIAS").as_deref() {
        Ok("max") => DropBias::Max,
        Ok("high") => DropBias::High,
        Ok("low") => DropBias::Low,
        Ok("min") => DropBias::Min,
        _ => DropBias::None,
    }
});

/// Apply bias to a c_int (for rand())
/// DISABLED - rand() affects too much game logic, causes crashes
fn apply_bias_int(value: c_int) -> c_int {
    // Don't bias rand() - it breaks game initialization
    value
}

// Track when the library was loaded
static INIT_TIME: Lazy<std::time::Instant> = Lazy::new(std::time::Instant::now);

// Delay before biasing starts (seconds) - let game initialize first
const BIAS_DELAY_SECS: u64 = 60;

/// Apply bias to bytes buffer (for getrandom())
/// Only biases 768-byte calls after startup delay
fn apply_bias_bytes(buf: *mut u8, len: usize) {
    if *RNG_BIAS == DropBias::None {
        return;
    }

    // Only bias 768-byte getrandom calls - these seem loot-related
    if len != 768 {
        return;
    }

    // Wait for game to fully initialize before biasing
    if INIT_TIME.elapsed().as_secs() < BIAS_DELAY_SECS {
        return;
    }

    let bytes = unsafe { std::slice::from_raw_parts_mut(buf, len) };
    for byte in bytes.iter_mut() {
        *byte = match *RNG_BIAS {
            DropBias::None => *byte,
            DropBias::Max => 255,
            DropBias::High => 192,
            DropBias::Low => 64,
            DropBias::Min => 0,
        };
    }
}

// Rate limiting - only log every Nth call to avoid spam
static CALL_COUNT: Lazy<Mutex<u64>> = Lazy::new(|| Mutex::new(0));

fn should_log_this_call() -> bool {
    if should_log_all() {
        return true;
    }

    let mut count = CALL_COUNT.lock().unwrap();
    *count += 1;

    // Log every 1000th call by default, or all if BL4_PRELOAD_ALL=1
    *count % 1000 == 0
}

fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn get_return_address() -> usize {
    // Get the return address (caller of our hook)
    let bt = Backtrace::new_unresolved();
    let frames: Vec<_> = bt.frames().iter().collect();

    // Frame 0 = this function
    // Frame 1 = our hook (rand, etc)
    // Frame 2 = actual caller we want
    if frames.len() > 2 {
        frames[2].ip() as usize
    } else {
        0
    }
}

fn log_call(func: &str, result: i64, return_addr: usize) {
    if !should_log_this_call() {
        return;
    }

    let timestamp = get_timestamp();

    let mut log = LOG_FILE.lock().unwrap();
    if let Some(ref mut file) = *log {
        let _ = writeln!(
            file,
            "{} | {} | ret={} | caller={:#x}",
            timestamp, func, result, return_addr
        );

        // Optionally log full stack trace
        if should_log_stacks() {
            let bt = Backtrace::new();
            let _ = writeln!(file, "{:?}", bt);
            let _ = writeln!(file, "---");
        }

        let _ = file.flush();
    }
}

// Hook for rand()
#[no_mangle]
pub unsafe extern "C" fn rand() -> c_int {
    let result = REAL_RAND();
    let biased = apply_bias_int(result);
    let return_addr = get_return_address();
    if biased != result {
        log_call(&format!("rand[{}->{}]", result, biased), biased as i64, return_addr);
    } else {
        log_call("rand", result as i64, return_addr);
    }
    biased
}

// Hook for srand()
#[no_mangle]
pub unsafe extern "C" fn srand(seed: c_uint) {
    let return_addr = get_return_address();
    log_call("srand", seed as i64, return_addr);
    REAL_SRAND(seed);
}

// Hook for random() (BSD-style)
#[no_mangle]
pub unsafe extern "C" fn random() -> c_long {
    let result = match *REAL_RANDOM {
        Some(func) => func(),
        None => rand() as c_long, // Fallback if random() doesn't exist
    };
    let return_addr = get_return_address();
    log_call("random", result as i64, return_addr);
    result
}

// Hook for arc4random() - common on BSD/macOS, sometimes available on Linux
#[no_mangle]
pub unsafe extern "C" fn arc4random() -> c_uint {
    // Try to find the real arc4random
    let ptr = libc::dlsym(libc::RTLD_NEXT, b"arc4random\0".as_ptr() as *const _);
    let result = if ptr.is_null() {
        // Fallback to rand if arc4random doesn't exist
        rand() as c_uint
    } else {
        let real_func: unsafe extern "C" fn() -> c_uint = std::mem::transmute(ptr);
        real_func()
    };

    let return_addr = get_return_address();
    log_call("arc4random", result as i64, return_addr);
    result
}

// Hook for getrandom() syscall wrapper - used by BCryptGenRandom via Wine
#[no_mangle]
pub unsafe extern "C" fn getrandom(buf: *mut c_void, buflen: usize, flags: c_uint) -> isize {
    let ptr = libc::dlsym(libc::RTLD_NEXT, b"getrandom\0".as_ptr() as *const _);
    let result = if ptr.is_null() {
        -1
    } else {
        let real_func: unsafe extern "C" fn(*mut c_void, usize, c_uint) -> isize =
            std::mem::transmute(ptr);
        real_func(buf, buflen, flags)
    };

    // Apply bias to the random bytes if successful
    if result > 0 && !buf.is_null() {
        apply_bias_bytes(buf as *mut u8, result as usize);
    }

    let return_addr = get_return_address();

    // Log first few bytes of random data for correlation
    let preview = if !buf.is_null() && buflen >= 4 && result > 0 {
        let bytes = std::slice::from_raw_parts(buf as *const u8, 4.min(buflen));
        format!("{:02x}{:02x}{:02x}{:02x}", bytes[0], bytes[1], bytes[2], bytes[3])
    } else {
        "????".to_string()
    };

    let bias_tag = if *RNG_BIAS != DropBias::None { "[BIASED]" } else { "" };
    log_call(&format!("getrandom({}b,{}){}", buflen, preview, bias_tag), result as i64, return_addr);
    result
}

// Constructor - runs when library is loaded
#[ctor::ctor]
fn init() {
    let log_path = get_log_path();

    // Create/clear log file
    if let Ok(mut file) = File::create(&log_path) {
        let _ = writeln!(file, "=== bl4-preload initialized ===");
        let _ = writeln!(file, "PID: {}", std::process::id());
        let _ = writeln!(file, "Timestamp: {}", get_timestamp());
        let _ = writeln!(file, "Log file: {}", log_path);
        let _ = writeln!(file, "BL4_PRELOAD_STACKS: {}", should_log_stacks());
        let _ = writeln!(file, "BL4_PRELOAD_ALL: {}", should_log_all());
        let _ = writeln!(file, "BL4_RNG_BIAS: {:?}", *RNG_BIAS);
        let _ = writeln!(file, "================================");
    }

    // Force initialization of function pointers
    let _ = *REAL_RAND;
    let _ = *REAL_SRAND;
    let _ = *REAL_RANDOM;
    let _ = *RNG_BIAS;
}
