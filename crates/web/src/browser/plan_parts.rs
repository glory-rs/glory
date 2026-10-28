//! Immutable physical addresses and current member ownership of one explicit
//! Plan. No DOM search, business values, callbacks or visited-row history.
use glory_html::{EditKind, Kind, Part, PlanShape, Property};
use glory_render::{Error, MAX_PLAN_NODES};

pub(crate) fn index(part: Part) -> usize {
    match part {
        Part::Main => 0,
        Part::Control => 1,
        Part::Content => 2,
        Part::Picker => 3,
    }
}

/// Immutable physical eligibility; live identity/placement still validates for
/// every submission. Prototypes retain this fact, never an instance's state.
pub(super) fn submission_shape(shape: &PlanShape) -> bool {
    shape.shapes.iter().all(|shape| {
        shape.edit == EditKind::None
            && shape.select_options.is_none()
            && !shape.pieces.is_empty()
            && shape.pieces.iter().all(|piece| match &piece.kind {
                Kind::Element {
                    attributes,
                    properties,
                    ..
                } => properties.iter().all(|(name, value)| {
                    *name == "disabled"
                        && matches!(value, Property::Bool(disabled) if *disabled == attributes.contains_key("disabled"))
                }),
                Kind::Text(_) => true,
            })
    })
}

pub(super) struct Layout {
    members: Vec<[Option<u32>; 4]>,
    #[cfg(target_arch = "wasm32")]
    parts: u32,
}
impl Layout {
    #[cfg(target_arch = "wasm32")]
    pub fn members(&self) -> usize {
        self.members.len()
    }
    #[cfg(target_arch = "wasm32")]
    pub fn parts(&self) -> u32 {
        self.parts
    }
    pub fn new(shapes: &PlanShape) -> Result<Self, Error> {
        if shapes.shapes.is_empty() || shapes.shapes.len() > MAX_PLAN_NODES {
            return Err(Error::InvalidProps("plan part member count"));
        }
        let mut offset = 0;
        let mut members = Vec::with_capacity(shapes.shapes.len());
        for shape in &shapes.shapes {
            shape.root_part()?;
            let mut parts = [None; 4];
            for piece in &shape.pieces {
                parts[index(piece.part)] = Some(offset);
                offset += 1;
            }
            members.push(parts);
        }
        Ok(Self {
            members,
            #[cfg(target_arch = "wasm32")]
            parts: offset,
        })
    }
    pub fn address(&self, member: usize, part: Part) -> Option<u32> {
        self.members[member][index(part)]
    }
}

const WORDS: usize = MAX_PLAN_NODES.div_ceil(u32::BITS as usize);

/// Fixed-size live membership, not a history of application keys. Both masks
/// belong to one live Plan instance; pending retirement clears on every flush.
pub(super) struct Retirement {
    live: [u32; WORDS],
    pending: [u32; WORDS],
}
impl Retirement {
    pub fn new(members: usize) -> Self {
        assert!((1..=MAX_PLAN_NODES).contains(&members));
        let mut live = [0; WORDS];
        for member in 0..members {
            live[member / 32] |= 1 << (member % 32);
        }
        Self {
            live,
            pending: [0; WORDS],
        }
    }
    pub fn retire(&mut self, member: usize) {
        let (word, bit) = (member / 32, 1 << (member % 32));
        assert_ne!(self.live[word] & bit, 0, "retired Plan member");
        self.live[word] &= !bit;
        self.pending[word] |= bit;
    }
    pub fn take(&mut self) -> Option<[u32; WORDS]> {
        let pending = std::mem::replace(&mut self.pending, [0; WORDS]);
        // When all members retire together, dropping their shared instance
        // releases the whole Array. Avoid one host clearing call per member.
        (self.live.iter().any(|word| *word != 0) && pending.iter().any(|word| *word != 0))
            .then_some(pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glory_html::shape;
    use glory_render::*;
    #[test]
    fn submission_rejects_editor_properties_ranges_and_disabled_disagreement() {
        let mut shapes = PlanShape {
            shapes: vec![shape(&Descriptor::new(Props::Button(Default::default()))).unwrap()],
            parents: vec![None],
        };
        assert!(submission_shape(&shapes));
        let piece = shapes.shapes[0]
            .pieces
            .iter_mut()
            .find(|piece| matches!(piece.kind, Kind::Element { .. }))
            .unwrap();
        let Kind::Element { properties, .. } = &mut piece.kind else {
            unreachable!()
        };
        properties.insert("disabled", Property::Bool(true));
        assert!(!submission_shape(&shapes));
        shapes.shapes[0] = shape(&Descriptor::new(Props::Input(Default::default()))).unwrap();
        assert!(!submission_shape(&shapes));
        shapes.shapes[0] = shape(&Descriptor::new(Props::Group)).unwrap();
        assert!(!submission_shape(&shapes));
    }
    #[test]
    fn physical_addresses_preserve_all_parts_including_control_roots() {
        let shapes = PlanShape {
            shapes: [
                Props::View,
                Props::Text(TextProps::default()),
                Props::File(FileProps::default()),
                Props::Input(InputProps::default()),
            ]
            .into_iter()
            .map(|props| shape(&Descriptor::new(props)).unwrap())
            .collect(),
            parents: vec![None, Some(0), Some(0), Some(0)],
        };
        let layout = Layout::new(&shapes).unwrap();
        let mut offset = 0;
        for (member, shape) in shapes.shapes.iter().enumerate() {
            for piece in &shape.pieces {
                assert_eq!(layout.address(member, piece.part), Some(offset));
                offset += 1;
            }
            for part in [Part::Main, Part::Control, Part::Content, Part::Picker] {
                assert_eq!(
                    layout.address(member, part).is_some(),
                    shape.pieces.iter().any(|piece| piece.part == part)
                );
            }
        }
        assert_eq!(shapes.shapes[2].root_part().unwrap(), Part::Control);
        let mut invalid = shapes.clone();
        let duplicate = invalid.shapes[0].pieces[0].clone();
        invalid.shapes[0].pieces.push(duplicate);
        assert!(Layout::new(&invalid).is_err());
        invalid.shapes[0] = shape(&Descriptor::new(Props::Group)).unwrap();
        assert!(Layout::new(&invalid).is_err());
    }
    #[test]
    fn partial_and_complete_retirement_cover_all_256_members_without_history() {
        for count in 1..=MAX_PLAN_NODES {
            let mut state = Retirement::new(count);
            assert_eq!(state.take(), None);
            for member in 0..count {
                state.retire(member);
                let mask = state.take();
                if member + 1 == count {
                    assert_eq!(mask, None);
                } else {
                    let mut expected = [0; WORDS];
                    expected[member / 32] = 1 << (member % 32);
                    assert_eq!(mask, Some(expected));
                }
                assert_eq!(state.take(), None);
            }
            let mut batch = Retirement::new(count);
            for member in 0..count {
                batch.retire(member);
            }
            assert_eq!(batch.take(), None);
        }
    }
}
