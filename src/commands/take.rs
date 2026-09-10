use std::{
    fs::{DirEntry, Metadata}, os::unix::{fs::MetadataExt}, path::{Path, PathBuf}, sync::{Arc, mpsc}
};

use crate::filter::should_ignore;
use crate::{error::Error, location::create_snapshot_name};

pub fn handle_take(tag: Option<crate::meta::RetentionTag>) -> Result<(), Error> {
    let (timestamp, snapshot_dir) = create_snapshot_name()?;

    let start = Path::new("/");

    let latest_location = crate::location::get_latest_location()?;
    //orchestrate_parallel_fs_walk(start, &snapshot_dir, &latest_location)?;
    linear_snapshot(start, &snapshot_dir, &latest_location)?;

    set_latest_symlink(&snapshot_dir)?;

    let tags = vec![tag.unwrap_or_default()];
    let metadata = crate::meta::SnapshotMetaData::new(timestamp, tags);
    metadata.append_to_metadata_file()?;

    Ok(())
}

fn linear_snapshot(path: &Path, snapshot_dir: &Path, latest_dir: &PathBuf) -> Result<(), Error> {
let mut stack = vec![path.to_path_buf()];

    while let Some(path) = stack.pop() {
        let entries = match std::fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            if file_type.is_symlink() {
                // FIXME: Symlinks need to be preserved,
                // when taking the snapshot.
                continue;

            } else if file_type.is_dir() && !should_ignore(&path) {
                stack.push(path.clone());

                let relative_path = path
                    .strip_prefix(std::path::Component::RootDir)
                    .map_err(std::io::Error::other)?;

                let target_dir = snapshot_dir.join(relative_path);

                std::fs::create_dir(target_dir)?;

            } else if file_type.is_file() {
                let relative_path = path
                    .strip_prefix(std::path::Component::RootDir)
                    .map_err(std::io::Error::other)?;

                let target_path = snapshot_dir.join(relative_path);
                incremental_copy(&entry, &target_path, latest_dir)?
            }
        }
    }

    Ok(())
}

// Update 'latest' symlink to reference a new snapshot,
// after removing the existing symlink.
fn set_latest_symlink(snapshot_dir: &Path) -> Result<(), Error> {
    let link = crate::location::get_latest_location()?;

    if link.exists() || std::fs::symlink_metadata(&link).is_ok() {
        std::fs::remove_file(&link)?;
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(snapshot_dir, &link)?;

    Ok(())
}

// Determines whether source is identical to target
// based on size and last modification time.
#[inline]
fn is_file_unchanged(source: &Metadata, target: &Metadata) -> bool {
    source.len() == target.len()
        && source.mtime() == target.mtime()
        && source.mtime_nsec() == target.mtime_nsec()

}

fn incremental_copy(source_dir: &DirEntry, target_dir: &Path, latest_dir: &PathBuf) -> Result<(), Error> {

    let source_dir_path = source_dir.path();

    let relative_source_dir = source_dir_path 
        .strip_prefix(std::path::Component::RootDir)
        .map_err(std::io::Error::other)?;

    //let latest_location = crate::location::get_latest_location()?;
    let previous_snapshot = latest_dir.join(relative_source_dir);

    // Rather than checking exists and then calling metadata, retrieve the
    // metadata and use this as a existence condition.
    let previous_snapshot_metadata = match std::fs::symlink_metadata(&previous_snapshot) {
        Ok(metadata) => Some(metadata),
        Err(_) => None,
    };

    let source_metadata = source_dir.metadata()?;
    if let Some(previous_metadata) = previous_snapshot_metadata && is_file_unchanged(&source_metadata, &previous_metadata) {
        std::fs::hard_link(&previous_snapshot, target_dir)?;

    } else {
    
        std::fs::copy(source_dir_path, target_dir)?;

        let permissions = source_metadata.permissions();
        std::fs::set_permissions(target_dir, permissions)?;

        if let (Ok(accessed), Ok(modified)) = (source_metadata.accessed(), source_metadata.modified()) {
            let times = std::fs::FileTimes::new()
                .set_accessed(accessed)
                .set_modified(modified);

            let target_buffer = std::fs::File::open(target_dir)?;
            target_buffer.set_times(times)?;

            //FIXME: std::fs::set_times is not yet stable.
            //For now, I'll have to open a buffer.
            //std::fs::set_times(target_dir, times)?;

            println!("{:?} -> {:?}", source_dir, target_dir);
        }
    }

    Ok(())
}



struct CopyTask {
    source_entry: DirEntry,
    target_path: PathBuf,
}

fn orchestrate_parallel_fs_walk(
    path: &Path,
    snapshot_dir: &Path,
    latest_dir: &PathBuf,
) -> Result<(), Error> {

    let (tx, rx) = mpsc::sync_channel::<CopyTask>(10_000);
    let rx = Arc::new(std::sync::Mutex::new(rx));

    let num_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8);

    let mut handles = vec![];

    for _ in 0..num_workers {
        let rx_clone = Arc::clone(&rx);
        let latest_dir = latest_dir.clone();

        let handle = std::thread::spawn(move || -> Result<(), Error> {
            loop {
                // Safely pop task from receiver
                let task = {
                    let lock = rx_clone.lock().unwrap();
                    match lock.recv() {
                        Ok(task) => task,
                        Err(_) => break,
                    }
                };

                let _ = incremental_copy(&task.source_entry, &task.target_path, &latest_dir);
            }
            Ok(())
        });
        handles.push(handle);
    }

    par_snapshot(path, snapshot_dir, &tx)?;

    drop(tx);

    for handle in handles {
        match handle.join() {
            Ok(res) => res?,
            Err(e) => std::panic::resume_unwind(e),
        }
    }

    Ok(())
}

fn par_snapshot(path: &Path, snapshot_dir: &Path, tx: &mpsc::SyncSender<CopyTask>) -> Result<(), Error> {
let mut stack = vec![path.to_path_buf()];

    while let Some(path) = stack.pop() {
        let entries = match std::fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            if file_type.is_symlink() {
                // FIXME: Symlinks need to be preserved,
                // when taking the snapshot.
                continue;

            } else if file_type.is_dir() && !should_ignore(&path) {
                stack.push(path.clone());

                let relative_path = path
                    .strip_prefix(std::path::Component::RootDir)
                    .map_err(std::io::Error::other)?;

                let target_dir = snapshot_dir.join(relative_path);

                std::fs::create_dir(target_dir)?;

            } else if file_type.is_file() {
                let relative_path = path
                    .strip_prefix(std::path::Component::RootDir)
                    .map_err(std::io::Error::other)?;

                let target_path = snapshot_dir.join(relative_path);
                //incremental_copy(&entry, &target_path, latest_dir)?
                
                let _ = tx.send(CopyTask {
                        source_entry: entry,
                        target_path,
                    });
            }
        }
    }

    Ok(())
}
