use crate::contract::*;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

// Python's JSON encoder uses shortest round-trip digits with different notation
// thresholds from serde_json. Preserve the value and its float/integer type.
struct PythonJson;
impl serde_json::ser::Formatter for PythonJson {
    fn write_f64<W: ?Sized + Write>(&mut self, writer: &mut W, value: f64) -> std::io::Result<()> {
        let number = serde_json::Number::from_f64(value)
            .ok_or_else(|| std::io::Error::other("nonfinite"))?;
        if value == 0.0 {
            return writer.write_all(if value.is_sign_negative() {
                b"-0.0"
            } else {
                b"0.0"
            });
        }
        let raw = number.to_string();
        let (sign, unsigned) = if let Some(s) = raw.strip_prefix('-') {
            ("-", s)
        } else {
            ("", raw.as_str())
        };
        let (mantissa, exponent) = if let Some((m, e)) = unsigned.split_once('e') {
            (m, e.parse::<i32>().expect("JSON number exponent"))
        } else {
            (unsigned, 0)
        };
        let mut point = mantissa.find('.').unwrap_or(mantissa.len()) as i32 + exponent;
        let digits = mantissa.replace('.', "");
        let leading = digits.len() - digits.trim_start_matches('0').len();
        point -= leading as i32;
        let digits = digits[leading..].trim_end_matches('0');
        let exponent = point - 1;
        let body = if !(-4..16).contains(&exponent) {
            let tail = if digits.len() > 1 {
                format!(".{}", &digits[1..])
            } else {
                String::new()
            };
            format!("{}{tail}e{exponent:+03}", &digits[..1])
        } else if point <= 0 {
            format!("0.{}{digits}", "0".repeat((-point) as usize))
        } else if point as usize >= digits.len() {
            format!("{digits}{}.0", "0".repeat(point as usize - digits.len()))
        } else {
            format!(
                "{}.{}",
                &digits[..point as usize],
                &digits[point as usize..]
            )
        };
        write!(writer, "{sign}{body}")
    }
}

pub fn encoded(value: &impl Serialize) -> Result<Vec<u8>> {
    // Sorted object keys, finite binary64 numbers and ASCII escapes match Python v1.
    let value = serde_json::to_value(value).map_err(|_| Stop::from("invalid_json"))?;
    let mut bytes = vec![];
    value
        .serialize(&mut serde_json::Serializer::with_formatter(
            &mut bytes, PythonJson,
        ))
        .map_err(|_| Stop::from("invalid_json"))?;
    let utf8 = String::from_utf8(bytes).map_err(|_| Stop::from("invalid_json"))?;
    let mut out = String::new();
    for ch in utf8.chars() {
        if (ch as u32) < 127 {
            out.push(ch);
        } else {
            for unit in ch.encode_utf16(&mut [0; 2]) {
                use std::fmt::Write;
                write!(out, "\\u{unit:04x}").expect("string write");
            }
        }
    }
    Ok(out.into_bytes())
}
pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn view_digest(view: &Value) -> Result<String> {
    Ok(sha256(&encoded(view)?))
}
pub fn load(path: &Path, limit: usize) -> Result<Value> {
    let file = File::open(path).map_err(|_| Stop::from("input_file"))?;
    let mut bytes = vec![];
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Stop::from("input_file"))?;
    require(bytes.len() <= limit, "input_size")?;
    decode(&bytes)
}

/// Reject duplicate keys at every depth before decoding typed envelopes.
pub fn decode(bytes: &[u8]) -> Result<Value> {
    use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
    struct Unique(Value);
    impl<'de> Deserialize<'de> for Unique {
        fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
            struct V;
            impl<'de> Visitor<'de> for V {
                type Value = Unique;
                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("unique JSON")
                }
                fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Unique, E> {
                    Ok(Unique(v.into()))
                }
                fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Unique, E> {
                    Ok(Unique(v.into()))
                }
                fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Unique, E> {
                    Ok(Unique(v.into()))
                }
                fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Unique, E> {
                    serde_json::Number::from_f64(v)
                        .map(|n| Unique(Value::Number(n)))
                        .ok_or_else(|| E::custom("nonfinite"))
                }
                fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Unique, E> {
                    Ok(Unique(v.into()))
                }
                fn visit_unit<E: de::Error>(self) -> std::result::Result<Unique, E> {
                    Ok(Unique(Value::Null))
                }
                fn visit_seq<A: SeqAccess<'de>>(
                    self,
                    mut a: A,
                ) -> std::result::Result<Unique, A::Error> {
                    let mut out = vec![];
                    while let Some(Unique(v)) = a.next_element()? {
                        out.push(v);
                    }
                    Ok(Unique(Value::Array(out)))
                }
                fn visit_map<A: MapAccess<'de>>(
                    self,
                    mut a: A,
                ) -> std::result::Result<Unique, A::Error> {
                    let mut out = serde_json::Map::new();
                    while let Some((k, Unique(v))) = a.next_entry::<String, Unique>()? {
                        if out.insert(k, v).is_some() {
                            return Err(de::Error::custom("duplicate"));
                        }
                    }
                    Ok(Unique(Value::Object(out)))
                }
            }
            d.deserialize_any(V)
        }
    }
    serde_json::from_slice::<Unique>(bytes)
        .map(|u| u.0)
        .map_err(|_| "invalid_json".into())
}

pub struct Evidence {
    path: PathBuf,
    events: File,
    pub used: usize,
    pub limit: usize,
    sequence: u32,
}
impl Evidence {
    pub const RESERVE: usize = 131072;
    pub fn create(path: &Path, limit: usize) -> Result<Self> {
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(path)
            .map_err(|_| Stop::from("output_exists_or_unavailable"))?;
        let events = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path.join("events.jsonl"))
            .map_err(|_| Stop::from("evidence_io"))?;
        Ok(Self {
            path: path.into(),
            events,
            used: 0,
            limit,
            sequence: 0,
        })
    }
    pub fn reserve(&self, amount: usize) -> Result<()> {
        require(
            self.used + amount <= self.limit - Self::RESERVE,
            "evidence_budget",
        )
    }
    pub fn event(&mut self, kind: &str, data: &impl Serialize) -> Result<()> {
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Stop::from("clock_error"))?
            .as_millis();
        let mut row = encoded(
            &json!({"version":1,"sequence":self.sequence,"at_unix_ms":at,"event":kind,"data":data}),
        )?;
        row.push(b'\n');
        self.reserve(row.len())?;
        self.events
            .write_all(&row)
            .and_then(|_| self.events.sync_all())
            .map_err(|_| Stop::from("evidence_io"))?;
        self.used += row.len();
        self.sequence += 1;
        Ok(())
    }
    pub fn finish(mut self, report: &Value, replay: &Replay) -> Result<()> {
        self.events.flush().map_err(|_| Stop::from("evidence_io"))?;
        let events =
            fs::read(self.path.join("events.jsonl")).map_err(|_| Stop::from("evidence_io"))?;
        let mut files = vec![
            ("report.json", encoded(report)?),
            ("replay.json", encoded(replay)?),
        ];
        let mut manifest = serde_json::Map::new();
        manifest.insert(
            "events.jsonl".into(),
            json!({"bytes":events.len(),"sha256":sha256(&events)}),
        );
        for (name, bytes) in &files {
            manifest.insert(
                (*name).into(),
                json!({"bytes":bytes.len(),"sha256":sha256(bytes)}),
            );
        }
        files.push(("artifacts.json", encoded(&manifest)?));
        require(
            self.used + files.iter().map(|(_, b)| b.len()).sum::<usize>() <= self.limit,
            "final_evidence_budget",
        )?;
        for (name, bytes) in files {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.path.join(name))
                .and_then(|mut f| {
                    f.write_all(&bytes)?;
                    f.sync_all()
                })
                .map_err(|_| Stop::from("evidence_io"))?;
        }
        Ok(())
    }
}
