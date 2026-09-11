use std::io::Write;

use crate::{error::Error, meta::RetentionTag, style::Colour};

const HOUR_IN_SECONDS: u64 = 60 * 60;
const DAY_IN_SECONDS: u64 = HOUR_IN_SECONDS * 24;
const WEEK_IN_SECONDS: u64 = DAY_IN_SECONDS * 7;

pub fn handle_list(
    display_size: bool,
    select_indexes: Option<Vec<usize>>,
    select_tags: Option<Vec<RetentionTag>>,
) -> Result<(), Error> {

    // TODO: the -s flag should append
    // to the normal list, and not
    // have a separate display.
    if display_size {
        crate::size::compute_par_snapshot_sizes()?;
        return Ok(());
    }

    let hostname = crate::location::get_hostname()?;
    let host_location = crate::location::get_host_location()?;
    let snapshots = crate::location::retrieve_snapshots(&host_location)?;

    let latest_location = crate::location::get_latest_location()?;
    let latest_location_target = std::fs::read_link(&latest_location)?;
    let latest_snapshot = latest_location_target.file_name().and_then(|s| s.to_str()?.parse::<u64>().ok());

    let metadata_file_content = crate::meta::read_metadata_file_to_string()?;
    let metadata = crate::meta::parse_metadata(&metadata_file_content)?;

    let stdout = std::io::stdout();
    let mut writer = std::io::BufWriter::new(stdout.lock());

    let mut index_iter = select_indexes.into_iter().flatten().peekable();

    for (i, snapshot) in snapshots.iter().enumerate() {

        let tags = metadata[i].tags();

        let (year, month, day, hour, min, sec) = epoch_to_datetime(*snapshot);

        let tag_string = crate::meta::RetentionTag::get_tags_mask(tags);

        // Check whether tags match.
        let mut is_selected = select_tags
            .clone()
            .is_some_and(|selected| selected.iter().any(|t| tags.contains(t)));

        if !is_selected && let Some(idx) = index_iter.peek() {
            // FIXME: This logic requires that select_indexes be sorted
            // ascending.
                if *idx == i {
                    index_iter.next();
                    is_selected = true;
                } else if *idx < i {
                    index_iter.next();
                }

        }

        let is_latest = latest_snapshot.is_some_and(|l| l == *snapshot);

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
