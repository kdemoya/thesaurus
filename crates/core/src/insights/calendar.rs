use super::*;
use chrono::{FixedOffset, NaiveDateTime, TimeZone};
fn stamp(value: &str, date_only: bool, offset: FixedOffset) -> Result<String> {
    if date_only {
        let day = NaiveDate::parse_from_str(value, "%Y%m%d").context("invalid iCalendar date")?;
        return Ok(offset
            .from_local_datetime(&day.and_hms_opt(0, 0, 0).unwrap())
            .single()
            .context("invalid calendar time")?
            .to_rfc3339());
    }
    let utc = value.ends_with('Z');
    let text = value.strip_suffix('Z').unwrap_or(value);
    let time = NaiveDateTime::parse_from_str(text, "%Y%m%dT%H%M%S")
        .context("invalid iCalendar DATE-TIME")?;
    let zone = if utc {
        FixedOffset::east_opt(0).unwrap()
    } else {
        offset
    };
    Ok(zone
        .from_local_datetime(&time)
        .single()
        .context("invalid calendar time")?
        .to_rfc3339())
}
fn text(value: &str) -> String {
    let mut result = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n' | 'N') => result.push('\n'),
                Some(next) => result.push(next),
                None => result.push('\\'),
            }
        } else {
            result.push(c)
        }
    }
    result
}
impl Store {
    /// RFC 5545 bounded occurrence import. Unsupported recurrence/TZID fails explicitly.
    pub fn import_capacity_calendar(req: CalendarImport) -> Result<CalendarImportPreview> {
        if req.ics.len() > 200000
            || req.ics.contains('\0')
            || !(-840..=840).contains(&req.utc_offset_minutes)
        {
            bail!("invalid calendar import limits");
        }
        let offset =
            FixedOffset::east_opt(req.utc_offset_minutes * 60).context("invalid UTC offset")?;
        let mut lines = Vec::<String>::new();
        for line in req.ics.lines() {
            let line = line.trim_end_matches('\r');
            if line.starts_with(' ') || line.starts_with('\t') {
                let previous = lines.last_mut().context("invalid folded calendar line")?;
                previous.push_str(&line[1..]);
            } else {
                lines.push(line.into());
            }
            if lines.len() > 20000 {
                bail!("invalid: calendar has too many lines");
            }
        }
        let mut stack = Vec::<String>::new();
        let mut fields = std::collections::BTreeMap::<String, (String, String)>::new();
        let mut events = Vec::new();
        let mut saw_calendar = false;
        let mut used_offset = false;
        for line in lines {
            if line.len() > 8000 {
                bail!("invalid: calendar property is too long");
            }
            let Some((left, value)) = line.split_once(':') else {
                if line.trim().is_empty() {
                    continue;
                }
                bail!("invalid iCalendar content line");
            };
            let header = left.to_ascii_uppercase();
            let left = header.as_str();
            let component = value.to_ascii_uppercase();
            let value = if left == "BEGIN" || left == "END" {
                component.as_str()
            } else {
                value
            };
            if left == "BEGIN" {
                if value == "VCALENDAR" {
                    if saw_calendar || !stack.is_empty() {
                        bail!("invalid calendar nesting");
                    }
                    saw_calendar = true;
                } else if stack.is_empty() {
                    bail!("invalid: component outside VCALENDAR");
                }
                if value == "VEVENT" {
                    if stack.last().map(String::as_str) != Some("VCALENDAR") {
                        bail!("invalid event nesting");
                    }
                    fields.clear();
                }
                stack.push(value.into());
                continue;
            }
            if left == "END" {
                if stack.pop().as_deref() != Some(value) {
                    bail!("invalid calendar component end");
                }
                if value != "VEVENT" {
                    continue;
                }
                if events.len() == 500 {
                    bail!("invalid: at most 500 calendar events");
                }
                if fields
                    .get("STATUS")
                    .is_some_and(|(_, v)| v.eq_ignore_ascii_case("CANCELLED"))
                {
                    continue;
                }
                let (start_params, start_value) = fields
                    .get("DTSTART")
                    .context("invalid: VEVENT requires DTSTART")?;
                let date_only = start_params.contains("VALUE=DATE")
                    && !start_params.contains("VALUE=DATE-TIME");
                for name in ["DTSTART", "DTEND"] {
                    if let Some((params, _)) = fields.get(name) {
                        if let Some(tzid) = params.split(';').find_map(|p| p.strip_prefix("TZID="))
                        {
                            if !["UTC", "ETC/UTC"].contains(&tzid.trim_matches('"')) {
                                bail!(
                                    "unsupported TZID {tzid}: export expanded UTC occurrences or enter commitments manually with explicit offsets"
                                );
                            }
                        }
                    }
                }
                let utc = start_value.ends_with('Z')
                    || start_params
                        .split(';')
                        .find_map(|p| p.strip_prefix("TZID="))
                        .is_some_and(|value| ["UTC", "ETC/UTC"].contains(&value.trim_matches('"')));
                let zone = if utc {
                    FixedOffset::east_opt(0).unwrap()
                } else {
                    used_offset = true;
                    offset
                };
                let starts_at = stamp(start_value, date_only, zone)?;
                let ends_at = if let Some((params, value)) = fields.get("DTEND") {
                    let end_date =
                        params.contains("VALUE=DATE") && !params.contains("VALUE=DATE-TIME");
                    if end_date != date_only {
                        bail!("invalid: DTSTART and DTEND value types differ");
                    }
                    let end_zone = if params
                        .split(';')
                        .find_map(|p| p.strip_prefix("TZID="))
                        .is_some_and(|value| ["UTC", "ETC/UTC"].contains(&value.trim_matches('"')))
                    {
                        FixedOffset::east_opt(0).unwrap()
                    } else if value.ends_with('Z') {
                        FixedOffset::east_opt(0).unwrap()
                    } else {
                        used_offset = true;
                        offset
                    };
                    stamp(value, end_date, end_zone)?
                } else if date_only {
                    let day = NaiveDate::parse_from_str(start_value, "%Y%m%d")?
                        .succ_opt()
                        .context("invalid all-day date")?;
                    stamp(&day.format("%Y%m%d").to_string(), true, zone)?
                } else {
                    bail!(
                        "unsupported: timed events require an explicit DTEND; enter a duration manually"
                    );
                };
                if DateTime::parse_from_rfc3339(&ends_at)?
                    <= DateTime::parse_from_rfc3339(&starts_at)?
                {
                    bail!("invalid: calendar end must follow start");
                }
                let uid = fields.get("UID").map(|(_, s)| s.as_str()).unwrap_or("");
                let title = fields
                    .get("SUMMARY")
                    .map(|(_, s)| text(s))
                    .unwrap_or_else(|| "Calendar commitment".into());
                validate_text(&title, 500, "calendar title")?;
                let id = crate::store::hash(&format!("{uid}:{starts_at}:{ends_at}:{title}"));
                if events
                    .iter()
                    .any(|event: &CalendarCommitment| event.id == id)
                {
                    continue;
                }
                let selected = !fields
                    .get("TRANSP")
                    .is_some_and(|(_, v)| v.eq_ignore_ascii_case("TRANSPARENT"));
                events.push(CalendarCommitment {
                    id,
                    title,
                    starts_at,
                    ends_at,
                    selected,
                    origin: "icalendar_import".into(),
                });
                continue;
            }
            if stack.last().map(String::as_str) != Some("VEVENT") {
                continue;
            }
            let name = left.split(';').next().unwrap_or("");
            if ["RRULE", "RDATE", "EXDATE", "RECURRENCE-ID", "DURATION"].contains(&name) {
                bail!(
                    "unsupported calendar property {name}: export concrete occurrences with DTSTART and DTEND; recurrence is never silently ignored"
                );
            }
            if ["DTSTART", "DTEND", "SUMMARY", "UID", "TRANSP", "STATUS"].contains(&name) {
                if fields
                    .insert(name.into(), (left.into(), value.into()))
                    .is_some()
                {
                    bail!("invalid: duplicate calendar property {name}");
                }
            }
        }
        if !saw_calendar || !stack.is_empty() {
            bail!("invalid: incomplete VCALENDAR");
        }
        let mut notes=vec!["Preview imported occurrences and explicitly select commitments before saving a capacity plan. Transparent events start unselected. Imported alarms are never executed. DTSTART/DTEND intervals use an exclusive end.".into()];
        if used_offset {
            notes.push(format!("Floating/all-day values use your explicit fixed UTC offset of {} minutes. This does not apply daylight-saving transitions; verify each event or import UTC occurrences.",req.utc_offset_minutes));
        }
        Ok(CalendarImportPreview { events, notes })
    }
}
