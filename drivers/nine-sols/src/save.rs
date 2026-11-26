//! A Nine Sols save: `flags.sav` is raw deflate around a MessagePack array of
//! three values — a version, the player's state as JSON text, and a map of
//! every flag of the game, keyed by the flag asset's id and type name.

use hundo_sdk::DriverError;
use serde_json::{Map, Value};

pub struct Save {
    pub player: Value,
    pub flags: Map<String, Value>,
}

/// Saves are a few hundred kilobytes; this leaves room for growth and no more.
const MAX_INFLATED: usize = 64 << 20;

pub fn parse(bytes: &[u8]) -> Result<Save, DriverError> {
    let raw = miniz_oxide::inflate::decompress_to_vec_with_limit(bytes, MAX_INFLATED)
        .map_err(|e| DriverError::Incomplete(format!("flags.sav: deflate {:?}", e.status)))?;
    let mut r = Reader { b: &raw, at: 0 };
    let n = r.array()?;
    if n < 3 {
        return Err(DriverError::Corrupt(format!("flags.sav: {n} values instead of 3")));
    }
    r.skip_int()?;
    let player = json(r.str()?)?;
    let flags = match r.value(0)? {
        Value::Object(m) => m,
        _ => return Err(DriverError::Corrupt("flags.sav: flags are not a map".into())),
    };
    Ok(Save { player, flags })
}

fn json(text: &str) -> Result<Value, DriverError> {
    serde_json::from_str(text).map_err(|e| DriverError::Corrupt(format!("flags.sav: {e}")))
}

/// MessagePack, as far as the save needs it, read into JSON values.
struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn byte(&mut self) -> Result<u8, DriverError> {
        let v = *self.b.get(self.at).ok_or_else(truncated)?;
        self.at += 1;
        Ok(v)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], DriverError> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.b.len()).ok_or_else(truncated)?;
        let v = &self.b[self.at..end];
        self.at = end;
        Ok(v)
    }

    fn be(&mut self, n: usize) -> Result<usize, DriverError> {
        Ok(self.take(n)?.iter().fold(0usize, |acc, &b| (acc << 8) | b as usize))
    }

    fn array(&mut self) -> Result<usize, DriverError> {
        match self.byte()? {
            t @ 0x90..=0x9f => Ok((t & 0x0f) as usize),
            0xdc => self.be(2),
            0xdd => self.be(4),
            t => Err(DriverError::Corrupt(format!("flags.sav: 0x{t:02x} where an array was expected"))),
        }
    }

    fn skip_int(&mut self) -> Result<(), DriverError> {
        match self.byte()? {
            0x00..=0x7f | 0xe0..=0xff => Ok(()),
            0xcc | 0xd0 => self.take(1).map(drop),
            0xcd | 0xd1 => self.take(2).map(drop),
            0xce | 0xd2 => self.take(4).map(drop),
            0xcf | 0xd3 => self.take(8).map(drop),
            t => Err(DriverError::Corrupt(format!("flags.sav: 0x{t:02x} where a number was expected"))),
        }
    }

    /// Any value; `depth` keeps a malformed save from recursing without end.
    fn value(&mut self, depth: u32) -> Result<Value, DriverError> {
        if depth > 32 {
            return Err(DriverError::Corrupt("flags.sav: nested too deep".into()));
        }
        let t = *self.b.get(self.at).ok_or_else(truncated)?;
        let len = |r: &mut Self, n| r.be(n);
        Ok(match t {
            0x00..=0x7f => { self.at += 1; Value::from(t) }
            0xe0..=0xff => { self.at += 1; Value::from(t as i8) }
            0xc0 => { self.at += 1; Value::Null }
            0xc2 | 0xc3 => { self.at += 1; Value::Bool(t == 0xc3) }
            0xcc..=0xcf => { self.at += 1; let n = 1 << (t - 0xcc); Value::from(len(self, n)? as u64) }
            0xd0..=0xd3 => {
                self.at += 1;
                let n = 1usize << (t - 0xd0);
                let v = len(self, n)? as u64;
                let shift = 64 - 8 * n as u32;
                Value::from(((v << shift) as i64) >> shift)
            }
            0xca => { self.at += 1; Value::from(f32::from_bits(len(self, 4)? as u32) as f64) }
            0xcb => { self.at += 1; Value::from(f64::from_bits(len(self, 8)? as u64)) }
            0xa0..=0xbf | 0xd9..=0xdb => Value::from(self.str()?),
            0xc4..=0xc6 => { self.at += 1; let n = len(self, 1 << (t - 0xc4))?; self.take(n)?; Value::Null }
            0x90..=0x9f | 0xdc | 0xdd => {
                let n = self.array()?;
                let mut out = Vec::with_capacity(n.min(1024));
                for _ in 0..n {
                    out.push(self.value(depth + 1)?);
                }
                Value::Array(out)
            }
            0x80..=0x8f | 0xde | 0xdf => {
                self.at += 1;
                let n = match t {
                    0xde => len(self, 2)?,
                    0xdf => len(self, 4)?,
                    _ => (t & 0x0f) as usize,
                };
                let mut out = Map::new();
                for _ in 0..n {
                    let k = match self.value(depth + 1)? {
                        Value::String(s) => s,
                        other => other.to_string(),
                    };
                    out.insert(k, self.value(depth + 1)?);
                }
                Value::Object(out)
            }
            t => return Err(DriverError::Corrupt(format!("flags.sav: unknown MessagePack type 0x{t:02x}"))),
        })
    }

    fn str(&mut self) -> Result<&'a str, DriverError> {
        let n = match self.byte()? {
            t @ 0xa0..=0xbf => (t & 0x1f) as usize,
            0xd9 => self.be(1)?,
            0xda => self.be(2)?,
            0xdb => self.be(4)?,
            t => return Err(DriverError::Corrupt(format!("flags.sav: 0x{t:02x} where text was expected"))),
        };
        std::str::from_utf8(self.take(n)?).map_err(|e| DriverError::Corrupt(format!("flags.sav: {e}")))
    }
}

fn truncated() -> DriverError {
    DriverError::Incomplete("flags.sav: truncated".into())
}
