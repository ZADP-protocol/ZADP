//! Local identity/auth files. Never put their contents in errors or logs.
use anyhow::{ensure, Result};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
use zadp_protocol::SellerKeyPair;
use zeroize::Zeroizing;

pub fn read_protected(path: &Path) -> Result<Zeroizing<Vec<u8>>> {
    read_protected_bounded(path, 4096)
}

/// Unsigned transaction proposals are larger than identity/authentication files.
/// Keep the same ownership, permissions and symlink checks with a bounded size.
pub fn read_protected_proposal(path: &Path) -> Result<Zeroizing<Vec<u8>>> {
    read_protected_bounded(path, 1_048_576)
}

fn read_protected_bounded(path: &Path, limit: u64) -> Result<Zeroizing<Vec<u8>>> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .map_err(|_| anyhow::anyhow!("cannot open protected local file"))?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() <= limit,
        "protected file is invalid or too large"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            metadata.mode() & 0o077 == 0,
            "protected file must not be accessible by group or other users (chmod 600)"
        );
        ensure!(
            metadata.uid() == unsafe_free_uid(),
            "protected file must be owned by current user"
        );
    }
    #[cfg(not(unix))]
    anyhow::bail!("protected file loading requires a Unix permission check in this MVP");
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "protected file grew beyond allowed size"
    );
    Ok(bytes)
}
#[cfg(unix)]
fn unsafe_free_uid() -> u32 {
    // /proc/self identifies the current process on Linux; no unsafe libc call.
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self")
        .map(|m| m.uid())
        .unwrap_or(u32::MAX)
}
pub fn load_identity(path: &Path) -> Result<SellerKeyPair> {
    let bytes = read_protected(path)?;
    let raw = Zeroizing::new(
        <[u8; 32]>::try_from(bytes.as_slice())
            .map_err(|_| anyhow::anyhow!("identity file must contain exactly 32 raw bytes"))?,
    );
    Ok(SellerKeyPair::from_secret_bytes(&raw))
}
pub fn write_identity(path: &Path, keys: &SellerKeyPair) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file: File = options
        .open(path)
        .map_err(|_| anyhow::anyhow!("identity path already exists or cannot be created"))?;
    let bytes = Zeroizing::new(keys.secret_bytes());
    file.write_all(bytes.as_ref())?;
    file.sync_all()?;
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::os::unix::{fs::symlink, fs::PermissionsExt};

    #[test]
    fn proposal_reader_preserves_protected_file_checks_and_size_bounds() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("proposal.json");
        std::fs::write(&path, vec![b' '; 6806]).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(read_protected(&path).is_err());
        assert_eq!(read_protected_proposal(&path).unwrap().len(), 6806);
        let link = directory.path().join("link.json");
        symlink(&path, &link).unwrap();
        assert!(read_protected_proposal(&link).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read_protected_proposal(&path).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::write(&path, vec![b' '; 1_048_577]).unwrap();
        assert!(read_protected_proposal(&path).is_err());
    }

    #[test]
    fn protected_identity_rejects_shared_files_symlinks_and_overwrite() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("seller.identity");
        let keys = SellerKeyPair::generate();
        write_identity(&path, &keys).unwrap();
        assert_eq!(load_identity(&path).unwrap().id, keys.id);
        assert!(write_identity(&path, &keys).is_err());
        let link = directory.path().join("link.identity");
        symlink(&path, &link).unwrap();
        assert!(load_identity(&link).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(load_identity(&path).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::write(&path, [0u8; 31]).unwrap();
        assert!(load_identity(&path).is_err());
    }
}
