use std::io::Write;

use crate::error::Error;

pub fn handle_remove(
    select_indexes: Option<Vec<usize>>,
    select_tags: Option<Vec<crate::meta::RetentionTag>>,
    force: bool,
) -> Result<(), Error> {
    let host_location = crate::location::get_host_location()?;
    let snapshots = crate::location::retrieve_snapshots(&host_location)?;
    let snapshot_count = snapshots.len();

    let latest_location = crate::location::get_latest_location()?;
    let mut latest_location_target = std::fs::read_link(&latest_location)?;
    let mut latest_snapshot = latest_location_target
        .file_name()
        .and_then(|s| s.to_str())
        .and_then(|s| s.parse::<u64>().ok());

    let metadata_file_content = crate::meta::read_metadata_file_to_string()?;
    let mut metadata = crate::meta::parse_metadata(&metadata_file_content)?;
    let metadata_count = metadata.len();

    let mut iter = snapshots.iter().enumerate().peekable();

    while let Some((i, snap)) = iter.next() {
        let index = snapshot_count - 1 - i;

        let tags = metadata[index].tags();

        let matches_index = select_indexes
            .as_ref()
            .is_some_and(|selected| selected.contains(&index));

        let matches_tag = select_tags
            .clone()
            .is_some_and(|selected| selected.iter().any(|t| tags.contains(t)));

        if matches_index || matches_tag {
            let is_latest = match latest_snapshot {
                Some(latest) => &latest == snap,
                None => false,
            };

            metadata.remove(index);

            let snapshot_directory = crate::location::construct_snapshot_directory(*snap)?;

            let mut input = if force {
                String::from("y")
            } else {
                String::new()
            };
            while input.trim().to_lowercase() != "y" && input.trim().to_lowercase() != "n" {
                input.clear();
                print!("Confirm removal of {:?}, (y/n): ", snapshot_directory);
                std::io::stdout().flush()?;
                std::io::stdin().read_line(&mut input)?;
            }

            if input.trim().to_lowercase() == "y" {
                println!("Removing: {:?}", snapshot_directory);

                if is_latest {
                    match iter.peek() {
                        Some((_, name_peek)) => {
                            let link = *name_peek;
                            let link_dirctory =
                                crate::location::construct_snapshot_directory(*link)?;
                            set_latest_symlink(&link_dirctory)?;
                            latest_snapshot = Some(*link);
                            latest_location_target = link_dirctory;
                        }
                        None => std::fs::remove_file(&latest_location_target)?,
                    }
                }
                remove_snapshot(&snapshot_directory)?;
            }
        }
    }

    if metadata_count != metadata.len() {
        crate::meta::SnapshotMetaData::overwrite_metadata_file(&metadata)?;
    }

    Ok(())
}

fn set_latest_symlink(snapshot_dir: &std::path::Path) -> Result<(), Error> {
    let link = crate::location::get_latest_location()?;

    if link.exists() || std::fs::symlink_metadata(&link).is_ok() {
        std::fs::remove_file(&link)?;
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(snapshot_dir, &link)?;

    Ok(())
}

fn remove_snapshot(snapshot_path: &std::path::Path) -> Result<(), Error> {
    let file_name = snapshot_path.file_name().unwrap().to_string_lossy();

    let parent = snapshot_path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid snapshot path")
    })?;

    let trash_dir = parent.join(".trash");
    std::fs::create_dir_all(&trash_dir)?;

    let trash_name = trash_dir.join(format!("tmp_{}", file_name));

    std::fs::rename(snapshot_path, &trash_name)?;

    std::fs::remove_dir_all(trash_name)?;

    Ok(())
}
