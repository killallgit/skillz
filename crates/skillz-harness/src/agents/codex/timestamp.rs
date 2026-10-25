use serde_json::Value;

/// `"timestamp": "2026-10-20T16:48:58.870Z"` of a session record, in ms since the epoch.
pub(super) fn record_ms(line: &str) -> Option<u64> {
    let record: Value = serde_json::from_str(line).ok()?;
    let ts = record["timestamp"].as_str()?.strip_suffix('Z')?;
    let (date, time) = ts.split_once('T')?;
    let (y, m, d) = parse_date(date)?;
    let (h, min, sec, millis) = parse_time(time)?;
    let seconds = (days_from_civil(y, m, d) * 24 + h) * 3600 + min * 60 + sec;
    u64::try_from(seconds * 1000 + millis).ok()
}

fn parse_date(date: &str) -> Option<(i64, i64, i64)> {
    let mut parts = date.split('-').map(|n| n.parse::<i64>());
    Some((
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    ))
}

fn parse_time(time: &str) -> Option<(i64, i64, i64, i64)> {
    let (hms, frac) = time.split_once('.').unwrap_or((time, "0"));
    let mut parts = hms.split(':').map(|n| n.parse::<i64>());
    let (h, min, sec) = (
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    );
    let millis = format!("{frac:0<3}")[..3].parse::<i64>().ok()?;
    Some((h, min, sec, millis))
}

/// Days since 1970-01-01 (Howard Hinnant's civil-from-days, inverted).
fn days_from_civil(y: i64, m: i64, day: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_timestamps_parse_to_epoch_ms() {
        let at = |ts: &str| record_ms(&serde_json::json!({"timestamp": ts}).to_string());
        assert_eq!(at("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(at("2026-10-20T16:48:58.870Z"), Some(1_792_514_938_870));
        assert_eq!(at("2024-02-29T12:00:00.5Z"), Some(1_709_208_000_500));
        assert_eq!(at("garbage"), None);
        assert_eq!(record_ms("not json"), None);
    }
}
