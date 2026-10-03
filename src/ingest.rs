use crate::domain::events::IngestEvent;
use std::{
    io::{self, Read},
    path::Path,
};

pub fn read_events(path: Option<&Path>) -> io::Result<Vec<IngestEvent>> {
    let reader: Box<dyn Read> = match path {
        Some(path) => Box::new(std::fs::File::open(path)?),
        None => Box::new(io::stdin()),
    };
    let mut input = String::new();
    reader
        .take(2 * 1024 * 1024 + 1)
        .read_to_string(&mut input)?;
    if input.len() > 2 * 1024 * 1024 {
        return Err(invalid("Input exceeds 2 MiB"));
    }
    parse_events(&input)
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub fn parse_events(input: &str) -> io::Result<Vec<IngestEvent>> {
    let input = input.trim_start_matches('\u{feff}');
    let values: Vec<serde_json::Value> = match serde_json::from_str(input) {
        Ok(serde_json::Value::Array(values)) => values,
        Ok(value) => vec![value],
        Err(_) => input
            .lines()
            .filter(|line| !line.trim().is_empty())
            .enumerate()
            .map(|(i, line)| {
                serde_json::from_str(line)
                    .map_err(|_| invalid(&format!("Invalid JSON on nonempty line {}", i + 1)))
            })
            .collect::<io::Result<_>>()?,
    };
    if values.is_empty() || values.len() > 500 {
        return Err(invalid("Import requires 1–500 events"));
    }
    values
        .into_iter()
        .map(|mut value| {
            // A stable fallback makes repeated imports idempotent. Producers should
            // supply event_id when two otherwise identical records are distinct.
            let hash = crate::redact::sha256_digest(&value.to_string());
            let object = value
                .as_object_mut()
                .ok_or_else(|| invalid("Each event must be a JSON object"))?;
            object
                .entry("event_id")
                .or_insert_with(|| format!("import_{hash}").into());
            let event: IngestEvent = serde_json::from_value(value)
                .map_err(|_| invalid("Invalid event envelope; see docs/EVENTS.md"))?;
            event.validate().map_err(|e| invalid(&e))?;
            Ok(event)
        })
        .collect()
}
