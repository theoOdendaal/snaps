use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::error::{Error, WithContext};

const BASE_SNAPSHOT_DIR: &str = concat!("/var/", env!("CARGO_PKG_NAME"), "/snapshots");

const HOSTNAME_FILE: &str = "/proc/sys/kernel/hostname";

pub fn get_hostname() -> Result<String, Error> {
    let hostname = std::fs::read_to_string(HOSTNAME_FILE)
        .with_context(HOSTNAME_FILE)?;

    Ok(hostname.trim().to_string())
}

pub fn get_host_location() -> Result<PathBuf, Error> {
    let hostname = std::fs::read_to_string(HOSTNAME_FILE)
        .with_context(HOSTNAME_FILE)?;

    let trimmed_hostname = hostname.trim();
    Ok(Path::new(BASE_SNAPSHOT_DIR).join(trimmed_hostname))
}

pub fn get_latest_location() -> Result<PathBuf, Error> {
    Ok(get_host_location()?.join("latest"))
}

pub fn create_snapshot_dir(timestamp: u64) -> Result<PathBuf, Error> {
    
    let path = construct_snapshot_directory_name(timestamp)?;

    if let Err(e) = std::fs::create_dir(&path) {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            return Err(Error::ExistingSnapshot(timestamp));
        }
        return Err(e.into());
    }
    Ok(path)
}

// Snapshots are initally created in a temporary
// directory, and only moved to the final
// directory once successful.
pub fn create_pending_snapshot_name() -> Result<(u64, PathBuf), Error> {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();

    let location = get_host_location()?
        .join(".pending")
        .join(timestamp.to_string());
    
    match std::fs::create_dir_all(&location) {
        Ok(_) => Ok((timestamp, location)),

        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(Error::ExistingSnapshot(timestamp))
        }

        Err(e) => Err(e.into()),
    }
}


pub fn construct_snapshot_directory_name(timestamp: u64) -> Result<PathBuf, Error> {
    let host_location = get_host_location()?;

    let directory = host_location.join(timestamp.to_string());

    Ok(directory)
}

pub fn retrieve_snapshots_as_ordered_vec(host_location: &Path) -> Result<Vec<u64>, Error> {
    let mut snapshots: Vec<u64> = Vec::with_capacity(30);

    for entry in std::fs::read_dir(host_location)? {
        let entry = entry?;

        match entry.file_type() {
            Ok(file_type) if file_type.is_dir() => match entry.file_name().to_str() {
                Some(file_name) if !file_name.starts_with(".") => {
                    let timestamp = file_name.parse::<u64>()?;
                    snapshots.push(timestamp);
                }
                _ => {}
            },
            _ => continue,
        }
    }
    snapshots.sort_unstable_by(|a, b| b.cmp(a));
    Ok(snapshots)
}

// Update 'latest' symlink to reference a new snapshot,
// after removing the existing symlink.
pub fn set_latest_symlink(snapshot_dir: &Path) -> Result<(), Error> {
    let link = crate::location::get_latest_location()?;

    let parent = link.parent().ok_or_else(|| Error::InvalidPath { path: link.clone() })?;

    let temp_link = parent.join(".latest-tmp");
    
    // In order to not causing racing condition problems.
    // The latest symlink is created atomically
    // using rename.
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&snapshot_dir, &temp_link)?;
        std::fs::rename(&temp_link, &link)?;
    }

    Ok(())
}

pub fn remove_latest_symlink() -> Result<(), Error> {
    let link = crate::location::get_latest_location()?;
    
    if link.exists() || std::fs::symlink_metadata(&link).is_ok() {
        std::fs::remove_file(&link)?;
    }
    Ok(())
}
