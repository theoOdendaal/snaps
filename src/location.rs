use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::error::Error;

const BASE_SNAPSHOT_DIR: &str = concat!("/var/", env!("CARGO_PKG_NAME"), "/snapshots");

const HOSTNAME_FILE: &str = "/proc/sys/kernel/hostname";

pub fn get_hostname() -> Result<String, Error> {
    let hostname = std::fs::read_to_string(HOSTNAME_FILE)?;
    Ok(hostname.trim().to_string())
}

pub fn get_host_location() -> Result<PathBuf, Error> {
    let hostname = std::fs::read_to_string(HOSTNAME_FILE)?;
    let trimmed_hostname = hostname.trim();
    Ok(Path::new(BASE_SNAPSHOT_DIR).join(trimmed_hostname))
}

pub fn get_latest_location() -> Result<PathBuf, Error> {
    Ok(get_host_location()?.join("latest"))
}

pub fn create_snapshot_name() -> Result<(u64, PathBuf), Error> {
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();

    let location = get_host_location()?.join(timestamp.to_string());
    
    match std::fs::create_dir(&location) {
        Ok(_) => Ok((timestamp, location)),

        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(Error::ExistingSnapshot(timestamp))
        }

        Err(e) => Err(e.into()),
    }
}

pub fn construct_snapshot_directory(snapshot: u64) -> Result<PathBuf, Error> {
    let host_location = get_host_location()?;
    let directory = host_location.join(snapshot.to_string());

    if !directory.is_dir() {
        return Err(Error::InvalidSnapshot(snapshot));
    }

    Ok(directory)
}

pub fn retrieve_snapshots(host_location: &Path) -> Result<Vec<u64>, Error> {
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
