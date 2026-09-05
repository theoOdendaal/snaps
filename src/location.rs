// File used to manage snapshot locations.

use crate::error::Error;

const BASE_SNAPSHOT_DIR: &str = concat!("/var/", env!("CARGO_PKG_NAME"), "/snapshots");
const HOSTNAME_FILE: &str = "/proc/sys/kernel/hostname";

// FIXME: Should hostname not rather be passed explicitly? Rather than auto retrieved?

pub fn get_host_name() -> Result<String, Error> {
    let hostname = std::fs::read_to_string(HOSTNAME_FILE)?;
    Ok(hostname.trim().to_string())
}

pub fn get_host_location() -> Result<std::path::PathBuf, Error> {
    let hostname = std::fs::read_to_string(HOSTNAME_FILE)?;
    let trimmed_hostname = hostname.trim();
    Ok(std::path::Path::new(BASE_SNAPSHOT_DIR).join(trimmed_hostname))
}

pub fn get_latest_location() -> Result<std::path::PathBuf, Error> {
    Ok(get_host_location()?.join("latest"))
}

pub fn construct_snapshot_directory(snapshot: u64) -> Result<std::path::PathBuf, Error> {
    let host_location = get_host_location()?;
    let directory = host_location.join(snapshot.to_string());

    if !directory.is_dir() {
        return Err(Error::InvalidSnapshot(snapshot));
    }

    Ok(directory)
}

pub fn retrieve_snapshot(host_location: &std::path::Path) -> Result<Vec<u64>, Error> {
    let mut snapshots: Vec<u64> = Vec::with_capacity(30);

    for entry in std::fs::read_dir(host_location)? {
        let entry = entry?;

        match entry.file_type() {
            Ok(file_type) if file_type.is_dir() => match entry.file_name().to_str() {
                Some(file_name) if !file_name.starts_with(".") => {
                    let secs = file_name.parse::<u64>()?;
                    snapshots.push(secs);
                }
                _ => {}
            },
            _ => continue,
        }
    }
    Ok(snapshots)
}
