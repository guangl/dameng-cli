use comfy_table::presets::UTF8_FULL;
use comfy_table::{ContentArrangement, Table};
use dameng_cli::PluginInfo;

const NON_TTY_WIDTH: u16 = 120;

/// Render installed plugins as a bordered UTF-8 table.
///
/// The table keeps the original empty-output behavior: when there are no
/// plugins, it returns an empty string instead of printing only a header.
pub fn render(plugins: &[PluginInfo]) -> String {
    if plugins.is_empty() {
        return String::new();
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_truncation_indicator("…")
        .set_header([
            "Name",
            "Version",
            "Description",
            "Source",
            "Revision",
            "Installed At",
        ]);

    // comfy-table auto-detects the terminal width only when stdout is a TTY.
    // Keep piped output deterministic and reasonably narrow as well.
    if !table.is_tty() {
        table.set_width(NON_TTY_WIDTH);
    }

    for plugin in plugins {
        let installed_at = format_installed_at(plugin.installed_at);
        table.add_row([
            plugin.manifest.name.as_str(),
            plugin.manifest.version.as_str(),
            plugin.manifest.description.as_str(),
            plugin.source.as_deref().unwrap_or("-"),
            plugin.revision.as_deref().unwrap_or("-"),
            installed_at.as_str(),
        ]);
    }

    table.to_string()
}

/// Format a Unix timestamp (seconds since the epoch) as UTC.
fn format_installed_at(unix_seconds: i64) -> String {
    let days = unix_seconds.div_euclid(86_400);
    let seconds_of_day = unix_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}Z")
}

/// Convert days since 1970-01-01 to a proleptic Gregorian calendar date.
///
/// This is Howard Hinnant's civil-from-days algorithm, adapted to work with
/// negative inputs as well.
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dameng_cli::{API_VERSION, Manifest};

    fn plugin_info(
        name: &str,
        version: &str,
        description: &str,
        source: Option<&str>,
        revision: Option<&str>,
        installed_at: i64,
    ) -> PluginInfo {
        PluginInfo {
            manifest: Manifest {
                name: name.to_owned(),
                version: version.to_owned(),
                description: description.to_owned(),
                api_version: API_VERSION,
                min_host_version: None,
                license: None,
                homepage: None,
                environment: Vec::new(),
                permissions: Vec::new(),
                hooks: Default::default(),
            },
            source: source.map(str::to_owned),
            revision: revision.map(str::to_owned),
            source_ref: None,
            checksum: String::new(),
            installed_at,
        }
    }

    #[test]
    fn empty_table_renders_empty() {
        assert_eq!(render(&[]), "");
    }

    #[test]
    fn table_contains_headers_and_rows() {
        let output = render(&[plugin_info(
            "probe",
            "0.1.0",
            "Test plugin",
            Some("/tmp/probe"),
            Some("deadbeef"),
            0,
        )]);

        for needle in [
            "Name",
            "Version",
            "Description",
            "Source",
            "Revision",
            "Installed At",
            "probe",
            "0.1.0",
            "Test plugin",
            "/tmp/probe",
            "deadbeef",
            "1970-01-01 00:00:00Z",
        ] {
            assert!(
                output.contains(needle),
                "missing {needle:?} in:
{output}"
            );
        }
    }

    #[test]
    fn formats_epoch_seconds_as_utc() {
        assert_eq!(format_installed_at(0), "1970-01-01 00:00:00Z");
        assert_eq!(format_installed_at(951_782_400), "2000-02-29 00:00:00Z");
        assert_eq!(format_installed_at(-1), "1969-12-31 23:59:59Z");
    }
}
