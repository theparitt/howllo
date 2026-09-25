// Bake the build timestamp into the binary so the running server can report
// exactly when the current code was compiled. Exposed as the HOWLLO_BUILT_AT
// compile-time env var (read with env!()).
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    // Recompile (and refresh the timestamp) whenever sources change.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=build.rs");

    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // RFC3339 UTC, computed without extra deps.
    println!("cargo:rustc-env=HOWLLO_BUILT_AT={}", format_rfc3339(secs));
}

/// Minimal epoch-seconds → "YYYY-MM-DDTHH:MM:SSZ" (UTC), no chrono in build.rs.
fn format_rfc3339(epoch_secs: u64) -> String {
    let days = epoch_secs / 86_400;
    let rem = epoch_secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };

    format!("{year:04}-{month:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}
