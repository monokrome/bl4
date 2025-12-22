//! Embedded Servo resources
//!
//! Provides ResourceReaderMethods implementation with resources embedded in the binary.

use std::path::PathBuf;

use embedder_traits::resources::{Resource, ResourceReaderMethods};

/// Embedded resource reader - all resources baked into binary
pub struct EmbeddedResourceReader;

impl EmbeddedResourceReader {
    pub fn new() -> Self {
        Self
    }

    /// Install this resource reader as Servo's global resource provider
    pub fn install() {
        embedder_traits::resources::set(Box::new(Self::new()));
    }
}

impl Default for EmbeddedResourceReader {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceReaderMethods for EmbeddedResourceReader {
    fn read(&self, res: Resource) -> Vec<u8> {
        match res {
            Resource::BluetoothBlocklist => {
                include_bytes!("../resources/gatt_blocklist.txt").to_vec()
            }
            Resource::DomainList => include_bytes!("../resources/public_domains.txt").to_vec(),
            Resource::HstsPreloadList => {
                include_bytes!("../resources/hsts_preload.fstmap").to_vec()
            }
            Resource::BadCertHTML => include_bytes!("../resources/badcert.html").to_vec(),
            Resource::NetErrorHTML => include_bytes!("../resources/neterror.html").to_vec(),
            Resource::BrokenImageIcon => include_bytes!("../resources/rippy.png").to_vec(),
            Resource::CrashHTML => include_bytes!("../resources/crash.html").to_vec(),
            Resource::DirectoryListingHTML => {
                include_bytes!("../resources/directory-listing.html").to_vec()
            }
            Resource::AboutMemoryHTML => include_bytes!("../resources/about-memory.html").to_vec(),
            Resource::DebuggerJS => include_bytes!("../resources/debugger.js").to_vec(),
        }
    }

    fn sandbox_access_files(&self) -> Vec<PathBuf> {
        // No sandbox file access needed for embedded resources
        vec![]
    }

    fn sandbox_access_files_dirs(&self) -> Vec<PathBuf> {
        // No sandbox directory access needed for embedded resources
        vec![]
    }
}
