use crate::error::{Error, Result};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Input,
    Output,
    Feature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub kind: Kind,
    pub id: u8,
    pub bits: u32,
}
impl Report {
    pub fn bytes(&self) -> u32 {
        self.bits.div_ceil(8) + u32::from(self.id != 0)
    }
}

#[derive(Debug)]
pub struct Descriptor {
    pub reports: Vec<Report>,
    pub usage_pages: BTreeSet<u32>,
}

#[derive(Default, Clone, Copy)]
struct Globals {
    size: u32,
    count: u32,
    id: u8,
}

impl Descriptor {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let bad = || Error::Protocol("malformed or excessive HID descriptor".into());
        if data.is_empty() || data.len() > 4096 {
            return Err(bad());
        }
        let mut state = Globals::default();
        let mut stack = Vec::new();
        let mut collections = 0u32;
        let mut result = Self {
            reports: Vec::new(),
            usage_pages: BTreeSet::new(),
        };
        let mut i = 0;
        while i < data.len() {
            let prefix = data[i];
            i += 1;
            if prefix == 0xfe {
                if i + 2 > data.len() {
                    return Err(bad());
                }
                let len = data[i] as usize;
                i += 2;
                if i + len > data.len() {
                    return Err(bad());
                }
                i += len;
                continue;
            }
            let size = match prefix & 3 {
                3 => 4,
                n => n as usize,
            };
            if i + size > data.len() {
                return Err(bad());
            }
            let mut value = 0u32;
            for n in 0..size {
                value |= (data[i + n] as u32) << (n * 8);
            }
            i += size;
            let item_type = (prefix >> 2) & 3;
            let tag = prefix >> 4;
            match (item_type, tag) {
                (1, 0) => {
                    result.usage_pages.insert(value);
                }
                (1, 7) => {
                    if value > 32768 {
                        return Err(bad());
                    }
                    state.size = value;
                }
                (1, 8) => {
                    if value == 0 || value > 255 {
                        return Err(bad());
                    }
                    state.id = value as u8;
                }
                (1, 9) => {
                    if value > 32768 {
                        return Err(bad());
                    }
                    state.count = value;
                }
                (1, 10) => {
                    if stack.len() >= 16 {
                        return Err(bad());
                    }
                    stack.push(state);
                }
                (1, 11) => {
                    state = stack.pop().ok_or_else(bad)?;
                }
                (0, 10) => {
                    collections += 1;
                    if collections > 32 {
                        return Err(bad());
                    }
                }
                (0, 12) => {
                    collections = collections.checked_sub(1).ok_or_else(bad)?;
                }
                (0, 8 | 9 | 11) => {
                    let kind = match tag {
                        8 => Kind::Input,
                        9 => Kind::Output,
                        _ => Kind::Feature,
                    };
                    let bits = state.size.checked_mul(state.count).ok_or_else(bad)?;
                    if bits == 0 || bits > 32768 {
                        return Err(bad());
                    }
                    if let Some(r) = result
                        .reports
                        .iter_mut()
                        .find(|r| r.id == state.id && r.kind == kind)
                    {
                        r.bits = r.bits.checked_add(bits).ok_or_else(bad)?;
                        if r.bits > 32768 {
                            return Err(bad());
                        }
                    } else {
                        result.reports.push(Report {
                            kind,
                            id: state.id,
                            bits,
                        });
                    }
                }
                _ => {}
            }
        }
        if collections != 0 || !stack.is_empty() || result.reports.is_empty() {
            return Err(bad());
        }
        result.reports.sort_by_key(|r| (r.id, r.kind));
        Ok(result)
    }

    pub fn hidpp_framing(&self) -> bool {
        let has = |kind, id, bytes| {
            self.reports
                .iter()
                .any(|r| r.kind == kind && r.id == id && r.bytes() == bytes)
        };
        has(Kind::Output, 0x10, 7) && (has(Kind::Input, 0x10, 7) || has(Kind::Input, 0x11, 20))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_model_is_adapted_by_framing_not_pid() {
        let d = Descriptor::parse(&[
            0x06, 0, 0xff, 0x09, 1, 0xa1, 1, 0x85, 0x10, 0x75, 8, 0x95, 6, 0x81, 2, 0x91, 2, 0x85,
            0x11, 0x95, 19, 0x81, 2, 0xc0,
        ])
        .unwrap();
        assert!(d.hidpp_framing());
        assert!(d.usage_pages.contains(&0xff00));
        assert_eq!(d.reports.len(), 3);
    }
    #[test]
    fn generic_keyboard_is_not_hidpp() {
        let d = Descriptor::parse(&[0x05, 1, 0x09, 6, 0xa1, 1, 0x75, 8, 0x95, 8, 0x81, 2, 0xc0])
            .unwrap();
        assert_eq!(d.reports[0].bytes(), 8);
        assert!(!d.hidpp_framing());
    }
    #[test]
    fn truncation_overflow_and_unbalanced_descriptors_fail() {
        for bytes in [
            vec![],
            vec![0x06, 0],
            vec![0xfe, 255, 0],
            vec![0xb4],
            vec![0xc0],
            vec![0x85, 0],
            vec![0x75, 8, 0x97, 0xff, 0xff, 0xff, 0xff, 0x81, 2],
        ] {
            assert!(Descriptor::parse(&bytes).is_err(), "{bytes:?}");
        }
    }
    #[test]
    fn report_fields_accumulate_and_global_push_pop_work() {
        let d = Descriptor::parse(&[
            0x75, 1, 0x95, 3, 0x81, 2, 0xa4, 0x95, 5, 0x81, 1, 0xb4, 0x81, 2,
        ])
        .unwrap();
        assert_eq!(d.reports[0].bits, 11);
        assert_eq!(d.reports[0].bytes(), 2);
    }
}
