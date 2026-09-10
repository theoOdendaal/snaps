use std::io::Write;

use crate::{error::Error, style::Colour, tags::SnapshotTag};

// FIXME: How would tagging of ad-hoc snapshots work?

const HOUR_IN_SECONDS: u64 = 60 * 60;
const DAY_IN_SECONDS: u64 = HOUR_IN_SECONDS * 24;
const WEEK_IN_SECONDS: u64 = DAY_IN_SECONDS * 7;

// TODO: Add tagging information !!! and other snapshot information, i.e. linux version etc. Add
// a cli flag for this?
pub fn handle_list(
    display_size: bool,
    select_indexes: Option<Vec<usize>>,
    select_tags: Option<Vec<SnapshotTag>>,
) -> Result<(), Error> {
    if display_size {
        crate::size::compute_par_snapshot_sizes()?;
        return Ok(());
    }

    let hostname = crate::location::get_host_name()?;
    let host_location = crate::location::get_host_location()?;
    let snapshots = crate::location::retrieve_snapshots(&host_location)?;

    let snapshot_count = snapshots.len();

    let latest_location = crate::location::get_latest_location()?;
    let latest_location_target = std::fs::read_link(&latest_location)?;
    let latest_snapshot = latest_location_target.file_name().and_then(|s| s.to_str());

    let stdout = std::io::stdout();
    let mut writer = std::io::BufWriter::new(stdout.lock());

    for i in 0..snapshot_count {
        let index = snapshot_count - 1 - i;
        let current_snapshot = snapshots[index];
        let (snapshot, tag) = (current_snapshot.timestamp(), current_snapshot.tag());
        let (year, month, day, hour, min, sec) = epoch_to_datetime(snapshot);

        let tag_string = if let Some(tag) = tag {
            tag.get_tag_mask()
        } else {
            "-----".into()
        };


        let mut is_selected = match select_tags.clone() {
            Some(selected_tags) if tag.as_ref().is_some_and(|tag| selected_tags.contains(tag)) => true,
            _ => false,
        };

        if let Some(indexes) = &select_indexes
            && indexes.contains(&i)
            && !is_selected
        {
            is_selected = true;
        }

        let is_latest = match latest_snapshot {
            Some(name) => name == current_snapshot.to_string(),
            None => false,
        };

        let latest_prefix = if is_latest { "[L]" } else { "" };

        if is_selected {
            write!(
                writer,
                "\x1b[{};{}m",
                Colour::Yellow.fg_code(),
                Colour::Blue.bg_code()
            )?;
        } else if is_latest {
            write!(writer, "\x1b[{}m", Colour::Cyan.fg_code())?;
        }

        let reset_code = if is_selected || is_latest {
            "\x1b[0m"
        } else {
            ""
        };

        writeln!(
            writer,
            "{hostname:<10} {tag_string:<6} {i:<3} {year:04}-{month:02}-{day:02} {hour:02}:{min:02}:{sec:02} {:<5} {snapshot} {latest_prefix} {reset_code}",
            snapshot / WEEK_IN_SECONDS,
        )?;
    }
    writer.flush()?;

    Ok(())
}

// https://howardhinnant.github.io/date_algorithms.html
pub fn epoch_to_datetime(secs: u64) -> (i32, u32, u32, u32, u32, u32) {
    let (days, time_secs) = (secs / DAY_IN_SECONDS, secs % DAY_IN_SECONDS);

    // Time component
    let (hours, remaining_secs) = (
        (time_secs / HOUR_IN_SECONDS) as u32,
        (time_secs % HOUR_IN_SECONDS) as u32,
    );
    let (minutes, seconds) = (remaining_secs / 60, remaining_secs % 60);

    // Date component
    let days = days as i64;
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146069;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (y as i32, m, d, hours, minutes, seconds)
}
