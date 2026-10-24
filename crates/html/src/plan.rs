use crate::{Kind, Property, Shape};
use glory_render::{Error, Plan, WeakPlan};
use std::{collections::HashMap, rc::Rc};

#[derive(Clone, Debug, PartialEq)]
pub struct PlanShape {
    pub shapes: Vec<Shape>,
    pub parents: Vec<Option<usize>>,
}
impl PlanShape {
    pub fn validate_instance(
        &self,
        plan: &Plan,
        ids: &[glory_render::NodeId],
        keys: &[u64],
    ) -> Result<(), Error> {
        let count = plan.nodes().len();
        if self.shapes.len() != count
            || self.parents.len() != count
            || ids.len() != count
            || keys.len() != count
            || self
                .parents
                .iter()
                .zip(plan.nodes())
                .any(|(parent, node)| *parent != node.parent)
        {
            return Err(Error::InvalidProps("plan shape/identity count or topology"));
        }
        Ok(())
    }
    /// Charged UTF-8 payload and physical node count, not measured heap bytes.
    pub fn weight(&self) -> (usize, usize) {
        let mut nodes = 0;
        let mut bytes = 0;
        for shape in &self.shapes {
            if let Some(options) = &shape.select_options {
                bytes += options.placeholder.as_ref().map_or(0, String::len);
                nodes += usize::from(options.placeholder.is_some());
                for entry in options.entries.iter() {
                    let opts = match entry {
                        glory_render::SelectEntry::Option(option) => std::slice::from_ref(option),
                        glory_render::SelectEntry::Group(group) => {
                            nodes += 1;
                            bytes += group.label.len();
                            group.options.as_slice()
                        }
                    };
                    for option in opts {
                        nodes += 1;
                        bytes +=
                            option.key.as_str().len() + option.label.len() + option.value.len();
                    }
                }
            }
            for piece in &shape.pieces {
                nodes += 1;
                match &piece.kind {
                    Kind::Text(text) => bytes += text.len(),
                    Kind::Element {
                        attributes,
                        properties,
                        ..
                    } => {
                        bytes += attributes.values().map(String::len).sum::<usize>();
                        bytes += properties
                            .values()
                            .map(|v| match v {
                                Property::Text(v) => v.len(),
                                Property::Bool(_) => 0,
                                Property::SelectedKeys(keys) => keys.iter().map(String::len).sum(),
                            })
                            .sum::<usize>();
                    }
                }
            }
        }
        (nodes, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glory_render::{Descriptor, PlanNode, Props};
    fn declaration() -> Plan {
        Plan::new(vec![PlanNode {
            parent: None,
            descriptor: Descriptor::new(Props::View),
        }])
        .unwrap()
    }
    #[test]
    fn shape_cache_shares_only_live_declarations_and_bounds_retained_entries() {
        let mut cache = Plans::default();
        let plan = declaration();
        let first = cache.shape(&plan).unwrap();
        assert!(Rc::ptr_eq(&first, &cache.shape(&plan).unwrap()));
        let plans: Vec<_> = (0..80).map(|_| declaration()).collect();
        for plan in &plans {
            cache.shape(plan).unwrap();
        }
        assert_eq!(cache.0.len(), 64);
        drop(plans);
        cache.shape(&declaration()).unwrap();
        assert_eq!(cache.0.len(), 2);
        drop(plan);
        cache.shape(&declaration()).unwrap();
        assert_eq!(cache.0.len(), 1);
    }
}
#[derive(Default)]
pub(crate) struct Plans(HashMap<usize, (WeakPlan, Rc<PlanShape>)>);
impl Plans {
    pub fn shape(&mut self, plan: &Plan) -> Result<Rc<PlanShape>, Error> {
        if let Some((source, shape)) = self.0.get(&plan.identity())
            && source.upgrade().is_some_and(|p| p.same(plan))
        {
            return Ok(shape.clone());
        }
        self.0.retain(|_, (source, _)| source.upgrade().is_some());
        let shape = Rc::new(PlanShape {
            shapes: plan
                .nodes()
                .iter()
                .map(|node| crate::shape(&node.descriptor))
                .collect::<Result<_, _>>()?,
            parents: plan.nodes().iter().map(|node| node.parent).collect(),
        });
        let (nodes, bytes) = self
            .0
            .values()
            .map(|(_, shape)| shape.weight())
            .fold(shape.weight(), |(an, ab), (n, b)| (an + n, ab + b));
        if self.0.len() < 64 && nodes <= 4096 && bytes <= 1024 * 1024 {
            self.0
                .insert(plan.identity(), (plan.downgrade(), shape.clone()));
        }
        Ok(shape)
    }
}
