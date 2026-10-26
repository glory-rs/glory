//! Fixed schema for a bounded, ordered field-write goal. This is not Rust's
//! struct layout: every u32 and UTF-16 string boundary is encoded explicitly.
use glory_html::{Edit, Part};
use glory_render::Error;

const LIMIT: usize = 16;
pub(super) struct Batch {
    records: [u32; LIMIT * 4],
    len: usize,
    strings: String,
}
impl Batch {
    pub fn new(
        edits: &[Edit],
        mut address: impl FnMut(Part) -> Option<u32>,
    ) -> Result<Self, Error> {
        let mut batch = Self {
            records: [0; LIMIT * 4],
            len: 0,
            strings: String::new(),
        };
        let mut end = 0_u32;
        for edit in edits.iter().take(LIMIT) {
            let (part, kind, name, value) = match edit {
                Edit::Text { part, value } => (*part, 0, "", value.as_str()),
                Edit::Attribute { part, name, value } => (
                    *part,
                    if value.is_some() { 1 } else { 2 },
                    *name,
                    value.as_deref().unwrap_or(""),
                ),
                _ => break,
            };
            let address = address(part).ok_or(Error::Unsupported("HTML part"))?;
            let name_end = end
                .checked_add(
                    u32::try_from(name.encode_utf16().count())
                        .map_err(|_| Error::InvalidProps("host field size"))?,
                )
                .ok_or(Error::InvalidProps("host field size"))?;
            end = name_end
                .checked_add(
                    u32::try_from(value.encode_utf16().count())
                        .map_err(|_| Error::InvalidProps("host field size"))?,
                )
                .ok_or(Error::InvalidProps("host field size"))?;
            batch.strings.push_str(name);
            batch.strings.push_str(value);
            batch.records[batch.len * 4..batch.len * 4 + 4]
                .copy_from_slice(&[address, kind, name_end, end]);
            batch.len += 1;
        }
        Ok(batch)
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn records(&self) -> &[u32] {
        &self.records[..self.len * 4]
    }
    pub fn strings(&self) -> &str {
        &self.strings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glory_html::Property;
    fn decode(batch: &Batch) -> Vec<Edit> {
        let strings: Vec<_> = batch.strings().encode_utf16().collect();
        let mut start = 0;
        batch
            .records()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|r| {
                let name_end = r[2] as usize;
                let end = r[3] as usize;
                let value = String::from_utf16(&strings[name_end..end]).unwrap();
                let name = String::from_utf16(&strings[start..name_end]).unwrap();
                start = end;
                let part = match r[0] {
                    5 => Part::Main,
                    9 => Part::Content,
                    _ => panic!("address"),
                };
                match r[1] {
                    0 => Edit::Text { part, value },
                    kind => {
                        assert_eq!(name, "aria-label");
                        Edit::Attribute {
                            part,
                            name: "aria-label",
                            value: (kind == 1).then_some(value),
                        }
                    }
                }
            })
            .collect()
    }
    #[test]
    fn utf16_boundaries_and_clear_keep_and_order_survive_the_wire() {
        let edits = [
            Edit::Text {
                part: Part::Content,
                value: "🙂a\0中".into(),
            },
            Edit::Attribute {
                part: Part::Main,
                name: "aria-label",
                value: Some("\"\\🙂".into()),
            },
            Edit::Attribute {
                part: Part::Main,
                name: "aria-label",
                value: None,
            },
            Edit::Attribute {
                part: Part::Main,
                name: "aria-label",
                value: Some(String::new()),
            },
        ];
        let batch =
            Batch::new(&edits, |part| Some(if part == Part::Main { 5 } else { 9 })).unwrap();
        assert_eq!(decode(&batch), edits);
        assert!(Batch::new(&edits, |_| None).is_err());
    }
    #[test]
    fn revision_and_controlled_properties_are_barriers_and_batches_are_bounded() {
        let text = Edit::Text {
            part: Part::Content,
            value: "x".into(),
        };
        for barrier in [
            Edit::Revision(7),
            Edit::Property {
                part: Part::Main,
                name: "value",
                value: Property::Text("draft".into()),
            },
        ] {
            let edits = [text.clone(), barrier, text.clone()];
            assert_eq!(Batch::new(&edits, |_| Some(9)).unwrap().len(), 1);
            assert_eq!(Batch::new(&edits[1..], |_| Some(9)).unwrap().len(), 0);
        }
        let edits = vec![text; LIMIT + 3];
        let batch = Batch::new(&edits, |_| Some(9)).unwrap();
        assert_eq!(batch.len(), LIMIT);
        assert_eq!(decode(&batch), edits[..LIMIT]);
    }
}
