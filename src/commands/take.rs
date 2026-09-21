use std::{
    fs::{DirEntry, Metadata}, os::unix::{fs::MetadataExt}, path::{Path, PathBuf}, sync::{Arc, mpsc}
};

use std::time::Instant;

use crate::{filter::should_ignore, location::create_pending_snapshot_name};
use crate::error::Error;

pub fn handle_take(tag: Option<crate::meta::RetentionTag>) -> Result<(), Error> {
    println!("[1/3] Creating pending directory...");
    let (timestamp, snapshot_dir) = create_pending_snapshot_name()?;
    println!("\tPending directory created: {}", snapshot_dir.display());

    println!("\n[2/3] Starting incremental snapshot...");
    let start = Path::new("/");
    let (_, host_path) = crate::location::get_host_dir_information()?;
    let latest_location = crate::location::get_latest_path(&host_path);
    //orchestrate_parallel_fs_walk(start, &snapshot_dir, &latest_location)?;
    linear_snapshot(start, &snapshot_dir, &latest_location)?;
    
    // Once once the snapshot has been successfully taken
    // in a temporary directory is it move to the 
    // main snapshot directory.
    println!("\n[3/3] Finalizing snapshot creation...");
    println!("\tMoving snapshot from pending");
    let final_snapshot_dir = crate::location::create_snapshot_dir(timestamp)?;
    std::fs::rename(&snapshot_dir, &final_snapshot_dir)?;
    
    println!("\tUpdating latest symlink");
    crate::location::set_latest_symlink(&final_snapshot_dir)?;

    println!("\tCreating snapshot metadata");
    let tags = vec![tag.unwrap_or_default()];
    let metadata = crate::meta::SnapshotMetaData::new(timestamp, tags)?;
    metadata.append_to_metadata_file()?;

    Ok(())
}

fn linear_snapshot(path: &Path, snapshot_dir: &Path, latest_dir: &Path) -> Result<(), Error> {
    let mut stack = vec![path.to_path_buf()];

    let mut progress_tracker = IncrementalCopyTracker::new();

    while let Some(path) = stack.pop() {
        let entries = match std::fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries {
            let entry = entry?;
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

                let target_path = snapshot_dir.join(relative_path);

                std::fs::create_dir(target_path)?;
                
                // FIXME: allocated bytes and delta bytes should
                // also be updated to account for dir sizes. Dir's
                // also have sizes, and this current approach
                // understates the total delta.

            } else if file_type.is_file() {
                let relative_path = path
                    .strip_prefix(std::path::Component::RootDir)
                    .map_err(std::io::Error::other)?;

                let target_path = snapshot_dir.join(relative_path);

                let copy_status = incremental_copy(&entry, &target_path, latest_dir)?;

                match copy_status {
                    IncrementalCopyStatus::Copied { allocated_bytes, apparent_bytes, delta_bytes } => {
                        progress_tracker.increase_allocated_bytes(allocated_bytes);
                        progress_tracker.increase_apparent_bytes(apparent_bytes);
                        progress_tracker.increase_delta_bytes(delta_bytes);
                    }
                    
                    IncrementalCopyStatus::Hardlinked { allocated_bytes, apparent_bytes } => {
                        progress_tracker.increase_allocated_bytes(allocated_bytes);
                        progress_tracker.increase_apparent_bytes(apparent_bytes);
                    }
                }

            }
        }

    }
    println!("{}", progress_tracker);

    Ok(())
}

// Determines whether source is identical to target
// based on size and last modification time.
#[inline]
fn is_file_unchanged(source: &Metadata, target: &Metadata) -> bool {
    source.len() == target.len()
        && source.mtime() == target.mtime()
        && source.mtime_nsec() == target.mtime_nsec()
        && source.mode() == target.mode()
        && source.uid() == target.uid()
        && source.gid() == target.gid()
}

pub struct IncrementalCopyTracker {
    start_time: Instant,
    allocated_bytes: u64,
    apparent_bytes: u64,
    delta_bytes: u64,
}

impl std::fmt::Display for IncrementalCopyTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let duration = self.start_time.elapsed().as_secs_f64();
        let fmt_allocated = crate::size::format_size(self.allocated_bytes) ;
        let fmt_apparent = crate::size::format_size(self.apparent_bytes);
        let fmt_delta = crate::size::format_size(self.delta_bytes);
        write!(f, "\tElapsed: {:.2}s\n\tAllocated size: {} \n\tApparent size: {} \n\tDelta size: {}", duration, fmt_allocated, fmt_apparent, fmt_delta)
    }
}

impl IncrementalCopyTracker {

    fn new() -> Self {
        let start_time = Instant::now();
        Self { start_time, allocated_bytes: 0, apparent_bytes: 0,  delta_bytes: 0 }
    }

    fn increase_apparent_bytes(&mut self, bytes: u64) {
        self.apparent_bytes+= bytes;
    }

    fn increase_allocated_bytes(&mut self, bytes: u64) {
        self.allocated_bytes += bytes;
    }

    fn increase_delta_bytes(&mut self, bytes: u64) {
        self.delta_bytes += bytes;
    }

}

enum IncrementalCopyStatus {
    // Adding delta bytes in order to allow
    // future copy logic to copy individual blocks
    // of data.
    Copied { allocated_bytes: u64, apparent_bytes: u64, delta_bytes: u64 },

    Hardlinked { allocated_bytes: u64, apparent_bytes: u64 },

}

fn incremental_copy(source_dir: &DirEntry, target_path: &Path, latest_dir: &Path) -> Result<IncrementalCopyStatus, Error> {
    let source_dir_path = source_dir.path();

    let relative_source_dir = source_dir_path 
        .strip_prefix(std::path::Component::RootDir)
        .map_err(std::io::Error::other)?;

    let previous_snapshot = latest_dir.join(relative_source_dir);

    // Rather than checking exists and then calling metadata, retrieve the
    // metadata and use this as a existence condition.
    let previous_snapshot_metadata = std::fs::symlink_metadata(&previous_snapshot).ok();

    let source_metadata = source_dir.metadata()?;
    if let Some(previous_metadata) = previous_snapshot_metadata && is_file_unchanged(&source_metadata, &previous_metadata) {

        std::fs::hard_link(&previous_snapshot, target_path)?;

        Ok(IncrementalCopyStatus::Hardlinked { allocated_bytes: previous_metadata.blocks() * 512, apparent_bytes: previous_metadata.len() })

    } else {
    
        std::fs::copy(source_dir_path, target_path)?;

        // Make sure to preserve owner and group details.
        std::os::unix::fs::chown(target_path, Some(source_metadata.uid()), Some(source_metadata.gid()))?;

        let permissions = source_metadata.permissions();
        std::fs::set_permissions(target_path, permissions)?;

        if let (Ok(accessed), Ok(modified)) = (source_metadata.accessed(), source_metadata.modified()) {
            let times = std::fs::FileTimes::new()
                .set_accessed(accessed)
                .set_modified(modified);

            let target_buffer = std::fs::File::open(target_path)?;
            target_buffer.set_times(times)?;

            //FIXME: std::fs::set_times is not yet stable.
            //For now, I'll have to open a buffer.
            //std::fs::set_times(target_path, times)?;

        }
        
        let size = source_metadata.blocks() * 512;
        Ok(IncrementalCopyStatus::Copied { allocated_bytes: size, apparent_bytes: source_metadata.len(), delta_bytes: size })
    }
}







struct CopyTask {
    source_entry: DirEntry,
    target_path: PathBuf,
}

fn orchestrate_parallel_fs_walk(
    path: &Path,
    snapshot_dir: &Path,
    latest_dir: &Path,
) -> Result<(), Error> {

    let (tx, rx) = mpsc::sync_channel::<CopyTask>(10_000);
    let rx = Arc::new(std::sync::Mutex::new(rx));

    let num_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8);

    let mut handles = vec![];

    for _ in 0..num_workers {
        let rx_clone = Arc::clone(&rx);
        let latest_dir = latest_dir.to_path_buf();

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
