use super::Repository;
use crate::{capture, redact::sha256_digest};
use serde_json::{json, Value};

fn attributes(value: &Value) -> serde_json::Map<String, Value> {
    let mut result = serde_json::Map::new();
    if let Some(items) = value.as_array() {
        for item in items {
            if let Some(key) = item["key"].as_str() {
                let v = &item["value"];
                result.insert(
                    key.into(),
                    v.get("stringValue")
                        .or(v.get("intValue"))
                        .or(v.get("boolValue"))
                        .cloned()
                        .unwrap_or_else(|| v.clone()),
                );
            }
        }
    }
    result
}
impl Repository {
    pub fn ingest_otlp(&self, batch: &Value) -> rusqlite::Result<usize> {
        self.transaction(||{
            let mut count=0;
            for (resources,scopes,items) in [("resourceLogs","scopeLogs","logRecords"),("resourceSpans","scopeSpans","spans")] {
                let Some(resource_list)=batch[resources].as_array() else{continue};
                for resource in resource_list {
                    let Some(scope_list)=resource[scopes].as_array() else{continue};
                    for scope in scope_list {
                        let Some(records)=scope[items].as_array() else{continue};
                        for record in records {
                            count+=1;if count>500{return Err(rusqlite::Error::InvalidParameterName("OTLP batch exceeds 500 records".into()))}
                            let mut attrs=attributes(&resource["resource"]["attributes"]);attrs.extend(attributes(&record["attributes"]));
                            let data=Value::Object(attrs);
                            let name=record["eventName"].as_str().or(data["event.name"].as_str()).or(record["name"].as_str()).unwrap_or("otel_event");
                            let record=json!({"type":name,"resource":resource["resource"],"scope":scope["scope"],"record":record});
                            let safe=if name.contains("tool_result"){capture::SafeContent::excluded("unpaired_otel_tool_output")}else{capture::sanitize(&record)};
                            let identity=sha256_digest(&safe.value().cloned().unwrap_or_else(||json!({"name":name,"time":record["record"]["timeUnixNano"],"trace":record["record"]["traceId"],"span":record["record"]["spanId"]})).to_string());
                            let mut input=capture::from_record("codex_otel","otlp_http",&identity,&record);
                            input.session_id=["conversation.id","thread.id","session.id","thread_id","session_id"].iter().find_map(|k|data[*k].as_str().map(capture::metadata));
                            input.turn_id=["turn.id","turn_id"].iter().find_map(|k|data[*k].as_str().map(capture::metadata));
                            input.source_version=data["service.version"].as_str().map(capture::metadata);
                            input.native_id=record["record"]["spanId"].as_str().map(capture::metadata);
                            input.observed_at=record["record"]["timeUnixNano"].as_str().and_then(|t|t.parse::<i64>().ok()).map(|n|chrono::DateTime::from_timestamp_nanos(n).to_rfc3339());
                            self.record_observation(&input,&safe)?;
                        }
                    }
                }
            }
            if count==0 {return Err(rusqlite::Error::InvalidParameterName("OTLP batch has no supported records".into()))}
            Ok(count)
        })
    }
}
