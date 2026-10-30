use crate::location::{decode, encode};
use crate::{Error, Limits, Location, Params, Result, RouteParams};
use std::{
    any::{Any, TypeId},
    collections::BTreeSet,
    marker::PhantomData,
    rc::Rc,
    sync::Arc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RouteId(pub &'static str);
pub struct RouteKey<P> {
    pub(crate) id: RouteId,
    marker: PhantomData<fn() -> P>,
}
impl<P> Copy for RouteKey<P> {}
impl<P> Clone for RouteKey<P> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<P> RouteKey<P> {
    pub const fn new(name: &'static str) -> Self {
        Self {
            id: RouteId(name),
            marker: PhantomData,
        }
    }
    pub fn id(self) -> RouteId {
        self.id
    }
}

type Decoder = fn(&Params) -> Result<Rc<dyn Any>>;
fn decoder<P: RouteParams>(params: &Params) -> Result<Rc<dyn Any>> {
    Ok(Rc::new(P::decode(params)?))
}

/// A declaration has no request state, UI closure, DOM object or HTTP type.
pub struct RouteDef {
    id: RouteId,
    pattern: String,
    kind: TypeId,
    decode: Decoder,
    children: Vec<RouteDef>,
    layout: bool,
}
impl RouteDef {
    pub fn new<P: RouteParams>(key: RouteKey<P>, pattern: impl Into<String>) -> Self {
        Self {
            id: key.id,
            pattern: pattern.into(),
            kind: TypeId::of::<P>(),
            decode: decoder::<P>,
            children: Vec::new(),
            layout: false,
        }
    }
    pub fn layout<P: RouteParams>(key: RouteKey<P>, pattern: impl Into<String>) -> Self {
        Self {
            layout: true,
            ..Self::new(key, pattern)
        }
    }
    pub fn child(mut self, child: RouteDef) -> Self {
        self.children.push(child);
        self
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlashPolicy {
    #[default]
    Strict,
    Ignore,
    RedirectToSlash,
    RedirectWithoutSlash,
}
#[derive(Clone, Debug)]
enum Segment {
    Static(String),
    Param(String),
    Optional(String),
    Rest(String),
}
impl Segment {
    fn name(&self) -> Option<&str> {
        match self {
            Self::Static(_) => None,
            Self::Param(name) | Self::Optional(name) | Self::Rest(name) => Some(name),
        }
    }
}
struct Node {
    id: RouteId,
    kind: TypeId,
    decode: Decoder,
    names: BTreeSet<String>,
    pattern: Vec<Segment>,
    trailing: bool,
}
struct Leaf {
    chain: Vec<usize>,
    pattern: Vec<Segment>,
    trailing: bool,
}
struct Table {
    nodes: Vec<Node>,
    leaves: Vec<Leaf>,
    base: String,
    limits: Limits,
    slash: SlashPolicy,
}
#[derive(Clone)]
pub struct Routes(Arc<Table>);
pub struct RoutesBuilder {
    roots: Vec<RouteDef>,
    base: String,
    limits: Limits,
    slash: SlashPolicy,
}
impl Default for RoutesBuilder {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            base: String::new(),
            limits: Limits::default(),
            slash: SlashPolicy::Strict,
        }
    }
}
impl RoutesBuilder {
    pub fn root(mut self, route: RouteDef) -> Self {
        self.roots.push(route);
        self
    }
    pub fn base(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();
        self
    }
    pub fn limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }
    pub fn slash_policy(mut self, policy: SlashPolicy) -> Self {
        self.slash = policy;
        self
    }
    pub fn finish(self) -> Result<Routes> {
        let base = if self.base.is_empty() || self.base == "/" {
            String::new()
        } else {
            let parsed = Location::parse_with(&self.base, self.limits)?;
            if parsed.path() != self.base
                || !parsed.query().0.is_empty()
                || parsed.fragment().is_some()
                || self.base.ends_with('/')
            {
                return Err(Error::Definition(
                    "base must be a canonical path without trailing slash".into(),
                ));
            }
            self.base
        };
        let mut table = Table {
            nodes: Vec::new(),
            leaves: Vec::new(),
            base,
            limits: self.limits,
            slash: self.slash,
        };
        let mut ids = BTreeSet::new();
        for root in self.roots {
            compile(
                root,
                &mut table,
                &mut ids,
                Vec::new(),
                Vec::new(),
                BTreeSet::new(),
            )?;
        }
        let mut signatures = BTreeSet::new();
        for leaf in &table.leaves {
            let signature = leaf
                .pattern
                .iter()
                .map(|part| match part {
                    Segment::Static(value) => format!("s:{}:{value}", value.len()),
                    Segment::Param(_) => "p".into(),
                    Segment::Optional(_) => "o".into(),
                    Segment::Rest(_) => "r".into(),
                })
                .collect::<Vec<_>>()
                .join("/");
            let trailing = self.slash == SlashPolicy::Strict && leaf.trailing;
            if !signatures.insert((signature, trailing)) {
                return Err(Error::Definition("ambiguous route patterns".into()));
            }
        }
        Ok(Routes(Arc::new(table)))
    }
}
fn pattern(value: &str) -> Result<Vec<Segment>> {
    if value.contains(['#', '\\']) || value.contains("//") {
        return Err(Error::Definition(format!("invalid pattern: {value}")));
    }
    let mut out = Vec::new();
    let trimmed = value.trim_matches('/');
    if trimmed.is_empty() {
        return Ok(out);
    }
    let parts: Vec<_> = trimmed.split('/').collect();
    for (index, value) in parts.iter().enumerate() {
        let segment = if let Some(name) = value.strip_prefix(':') {
            if let Some(name) = name.strip_suffix('?') {
                if index + 1 != parts.len() {
                    return Err(Error::Definition("optional parameter must be last".into()));
                }
                Segment::Optional(name.into())
            } else {
                Segment::Param(name.into())
            }
        } else if let Some(name) = value.strip_prefix('*') {
            if index + 1 != parts.len() {
                return Err(Error::Definition("catch-all must be last".into()));
            }
            Segment::Rest(name.into())
        } else {
            if value.contains(['?', ':', '*']) || *value == "." || *value == ".." {
                return Err(Error::Definition("invalid static segment".into()));
            }
            Segment::Static(decode(value)?)
        };
        if let Some(name) = segment.name()
            && (name.is_empty()
                || !name
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_'))
        {
            return Err(Error::Definition("invalid parameter name".into()));
        }
        out.push(segment);
    }
    Ok(out)
}
fn compile(
    def: RouteDef,
    table: &mut Table,
    ids: &mut BTreeSet<RouteId>,
    mut chain: Vec<usize>,
    mut full: Vec<Segment>,
    mut names: BTreeSet<String>,
) -> Result {
    if table.nodes.len() >= table.limits.routes || chain.len() >= table.limits.depth {
        return Err(Error::OverLimit("route declaration"));
    }
    if def.id.0.is_empty()
        || def.id.0.len() > 128
        || def.id.0.chars().any(char::is_control)
        || !ids.insert(def.id)
    {
        return Err(Error::Definition("duplicate/invalid route id".into()));
    }
    if full.iter().any(|part| matches!(part, Segment::Rest(_))) {
        return Err(Error::Definition("children after catch-all".into()));
    }
    let own = pattern(&def.pattern)?;
    for part in &own {
        if let Some(name) = part.name()
            && !names.insert(name.into())
        {
            return Err(Error::Definition(format!(
                "duplicate path parameter: {name}"
            )));
        }
    }
    full.extend(own);
    if full.len() > table.limits.segments {
        return Err(Error::OverLimit("pattern segments"));
    }
    let trailing = def.pattern.ends_with('/') && !full.is_empty();
    chain.push(table.nodes.len());
    table.nodes.push(Node {
        id: def.id,
        kind: def.kind,
        decode: def.decode,
        names: names.clone(),
        pattern: full.clone(),
        trailing,
    });
    if def.children.is_empty() {
        if def.layout {
            return Err(Error::Definition(
                "layout requires a child/index route".into(),
            ));
        }
        table.leaves.push(Leaf {
            chain,
            pattern: full,
            trailing,
        });
    } else {
        if !def.layout {
            return Err(Error::Definition("a parent must be a layout".into()));
        }
        for child in def.children {
            compile(
                child,
                table,
                ids,
                chain.clone(),
                full.clone(),
                names.clone(),
            )?;
        }
    }
    Ok(())
}

#[derive(Clone)]
pub struct Match {
    pub id: RouteId,
    pub params: Params,
    pub(crate) value: Rc<dyn Any>,
}
impl Match {
    pub fn typed<P: RouteParams>(&self, key: RouteKey<P>) -> Result<Rc<P>> {
        if self.id != key.id {
            return Err(Error::UnknownRoute(key.id.0.into()));
        }
        self.value
            .clone()
            .downcast()
            .map_err(|_| Error::WrongParamType)
    }
}
#[derive(Clone, Default)]
pub struct MatchPlan {
    pub chain: Vec<Match>,
}
impl MatchPlan {
    pub fn params<P: RouteParams>(&self, key: RouteKey<P>) -> Result<Rc<P>> {
        self.chain
            .iter()
            .find(|entry| entry.id == key.id)
            .ok_or_else(|| Error::UnknownRoute(key.id.0.into()))?
            .typed(key)
    }
}

impl Routes {
    #[cfg(feature = "widgets")]
    pub(crate) fn same_table(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn builder() -> RoutesBuilder {
        RoutesBuilder::default()
    }
    pub fn base(&self) -> &str {
        &self.0.base
    }
    pub fn limits(&self) -> Limits {
        self.0.limits
    }
    /// Deterministic declaration identity. Applications must bump codec_schema
    /// when changing a codec without changing its path pattern. Not a MAC.
    pub fn schema(&self, codec_schema: &str) -> String {
        let mut hash = 0xcbf29ce484222325u64;
        let mut add = |value: &str| {
            for byte in value.as_bytes().iter().copied().chain([0]) {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
        };
        add(codec_schema);
        add(&self.0.base);
        add(&format!("{:?}", self.0.slash));
        for leaf in &self.0.leaves {
            for index in &leaf.chain {
                add(self.0.nodes[*index].id.0);
            }
            add(&format!("{:?}/{}", leaf.pattern, leaf.trailing));
        }
        format!("{hash:016x}")
    }
    pub fn contains<P: 'static>(&self, key: RouteKey<P>) -> bool {
        self.0
            .nodes
            .iter()
            .any(|node| node.id == key.id && node.kind == TypeId::of::<P>())
    }
    pub fn canonical(&self, location: &Location) -> Location {
        let path = location.path();
        match self.0.slash {
            SlashPolicy::RedirectToSlash if path != "/" && !path.ends_with('/') => {
                location.with_path(format!("{path}/"))
            }
            SlashPolicy::RedirectWithoutSlash if path != "/" && path.ends_with('/') => {
                location.with_path(path.trim_end_matches('/').into())
            }
            _ => location.clone(),
        }
    }
    pub fn resolve(&self, location: &Location) -> Result<MatchPlan> {
        let validated = Location::parse_with(&location.to_string(), self.0.limits)?;
        let path = validated.path();
        let path = path
            .strip_prefix(&self.0.base)
            .filter(|rest| rest.is_empty() || rest.starts_with('/'))
            .ok_or(Error::OutsideBase)?;
        let relative =
            Location::parse_with(if path.is_empty() { "/" } else { path }, self.0.limits)?;
        let values = relative.segments()?;
        let trailing = path.ends_with('/') && !values.is_empty();
        let mut steps = 0;
        let mut best: Option<(&Leaf, Params, Vec<u8>, Omissions)> = None;
        for leaf in &self.0.leaves {
            if self.0.slash == SlashPolicy::Strict && trailing != leaf.trailing {
                continue;
            }
            if let Some((params, ranks, penalty)) = match_parts(
                &leaf.pattern,
                &values,
                Params::default(),
                Vec::new(),
                Omissions::default(),
                &mut steps,
                self.0.limits.match_steps,
            )? && best.as_ref().is_none_or(|(_, _, old_rank, old_penalty)| {
                (&ranks, std::cmp::Reverse(penalty)) > (old_rank, std::cmp::Reverse(*old_penalty))
            }) {
                best = Some((leaf, params, ranks, penalty));
            }
        }
        let (leaf, params, _, _) = best.ok_or(Error::NoMatch)?;
        let chain = leaf
            .chain
            .iter()
            .map(|index| {
                let node = &self.0.nodes[*index];
                let params = Params(
                    params
                        .0
                        .iter()
                        .filter(|(key, _)| node.names.contains(*key))
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect(),
                );
                Ok(Match {
                    id: node.id,
                    value: (node.decode)(&params)?,
                    params,
                })
            })
            .collect::<Result<_>>()?;
        Ok(MatchPlan { chain })
    }
    pub fn target<P: RouteParams>(&self, key: RouteKey<P>, params: P) -> Result<Location> {
        let node = self
            .0
            .nodes
            .iter()
            .find(|node| node.id == key.id)
            .ok_or_else(|| Error::UnknownRoute(key.id.0.into()))?;
        if node.kind != TypeId::of::<P>() {
            return Err(Error::WrongParamType);
        }
        let values = params.encode()?;
        if values.0.keys().any(|name| !node.names.contains(name)) {
            return Err(Error::BadParams("unknown parameter".into()));
        }
        let mut path = self.0.base.clone();
        for part in &node.pattern {
            let segments: Vec<&str> = match part {
                Segment::Static(value) => vec![value],
                Segment::Param(name) => {
                    let values = values
                        .0
                        .get(name)
                        .filter(|values| values.len() == 1 && !values[0].is_empty())
                        .ok_or_else(|| Error::BadParams(name.clone()))?;
                    vec![values[0].as_str()]
                }
                Segment::Optional(name) => match values.0.get(name) {
                    None => Vec::new(),
                    Some(values) if values.len() == 1 && !values[0].is_empty() => {
                        vec![values[0].as_str()]
                    }
                    _ => return Err(Error::BadParams(name.clone())),
                },
                Segment::Rest(name) => values
                    .0
                    .get(name)
                    .ok_or_else(|| Error::BadParams(name.clone()))?
                    .iter()
                    .map(String::as_str)
                    .collect(),
            };
            for segment in segments {
                if segment.is_empty() || segment == "." || segment == ".." {
                    return Err(Error::BadParams("empty/dot segment".into()));
                }
                path.push('/');
                path.push_str(&encode(segment));
            }
        }
        if path.is_empty() || node.trailing {
            path.push('/');
        }
        let location = self.canonical(&Location::parse_with(&path, self.0.limits)?);
        // Encode/decode must agree, and a more specific route must not steal this target.
        let matched = self.resolve(&location)?;
        if !matched.chain.iter().any(|entry| entry.id == key.id) {
            return Err(Error::BadParams("target matches another route".into()));
        }
        Ok(location)
    }
    /// Resolve a route-relative URL against this node's consumed path. Unlike
    /// a browser anchor, a child path appends to the route, not its parent file.
    pub fn resolve_relative(
        &self,
        id: RouteId,
        current: &Location,
        relative: &str,
    ) -> Result<Location> {
        if relative.contains('\\') || relative.chars().any(char::is_control) {
            return Err(Error::BadUrl("invalid relative target".into()));
        }
        let plan = self.resolve(current)?;
        let matched = plan
            .chain
            .iter()
            .find(|matched| matched.id == id)
            .ok_or(Error::Gone)?;
        let node = self
            .0
            .nodes
            .iter()
            .find(|node| node.id == id)
            .ok_or(Error::Gone)?;
        let count = node
            .pattern
            .iter()
            .map(|segment| match segment {
                Segment::Static(_) | Segment::Param(_) => 1,
                Segment::Optional(name) => usize::from(matched.params.0.contains_key(name)),
                Segment::Rest(name) => matched.params.0.get(name).map_or(0, Vec::len),
            })
            .sum::<usize>();
        let base_count = self
            .0
            .base
            .split('/')
            .filter(|part| !part.is_empty())
            .count();
        let mut prefix = format!(
            "/{}",
            current
                .path()
                .trim_matches('/')
                .split('/')
                .take(base_count + count)
                .collect::<Vec<_>>()
                .join("/")
        );
        if node.trailing && prefix != "/" {
            prefix.push('/');
        }
        let base = if relative.starts_with(['?', '#']) || relative.is_empty() {
            let query = if relative.starts_with('#') || relative.is_empty() {
                current.with_path(prefix).without_fragment().to_string()
            } else {
                prefix
            };
            format!("https://glory.invalid{query}")
        } else {
            format!("https://glory.invalid{}/", prefix.trim_end_matches('/'))
        };
        let url = url::Url::parse(&base)
            .and_then(|base| base.join(relative))
            .map_err(|error| Error::BadUrl(error.to_string()))?;
        let url = url
            .as_str()
            .strip_prefix("https://glory.invalid")
            .filter(|rest| rest.starts_with('/'))
            .ok_or(Error::Unsupported("external relative target"))?;
        let location = Location::parse_with(url, self.0.limits)?;
        if !location
            .path()
            .strip_prefix(&self.0.base)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        {
            return Err(Error::OutsideBase);
        }
        Ok(location)
    }
    pub fn static_locations(&self) -> Result<Vec<Location>> {
        self.0
            .leaves
            .iter()
            .filter(|leaf| {
                leaf.pattern
                    .iter()
                    .all(|part| matches!(part, Segment::Static(_)))
            })
            .map(|leaf| {
                let mut path = self.0.base.clone();
                for part in &leaf.pattern {
                    if let Segment::Static(value) = part {
                        path.push('/');
                        path.push_str(&encode(value));
                    }
                }
                if path.is_empty() || leaf.trailing {
                    path.push('/');
                }
                Ok(self.canonical(&Location::parse(&path)?))
            })
            .collect()
    }
}
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Omissions {
    empty_rest: bool,
    optional: usize,
}
type Captured = Option<(Params, Vec<u8>, Omissions)>;
fn match_parts(
    parts: &[Segment],
    values: &[String],
    mut params: Params,
    mut ranks: Vec<u8>,
    penalty: Omissions,
    steps: &mut usize,
    limit: usize,
) -> Result<Captured> {
    *steps += 1;
    if *steps > limit {
        return Err(Error::OverLimit("matching steps"));
    }
    let Some((first, rest)) = parts.split_first() else {
        return Ok(values.is_empty().then_some((params, ranks, penalty)));
    };
    match first {
        Segment::Rest(name) => {
            params.0.insert(name.clone(), values.to_vec());
            ranks.extend(std::iter::repeat_n(1, values.len()));
            Ok(Some((
                params,
                ranks,
                Omissions {
                    empty_rest: values.is_empty(),
                    ..penalty
                },
            )))
        }
        Segment::Optional(name) => {
            if let Some((value, tail)) = values.split_first()
                && !value.is_empty()
            {
                let mut captured = params.clone();
                captured.insert(name, value);
                let mut rank = ranks.clone();
                rank.push(2);
                if let Some(result) =
                    match_parts(rest, tail, captured, rank, penalty, steps, limit)?
                {
                    return Ok(Some(result));
                }
            }
            match_parts(
                rest,
                values,
                params,
                ranks,
                Omissions {
                    optional: penalty.optional + 1,
                    ..penalty
                },
                steps,
                limit,
            )
        }
        Segment::Param(name) => {
            let Some((value, tail)) = values.split_first().filter(|(value, _)| !value.is_empty())
            else {
                return Ok(None);
            };
            params.insert(name, value);
            ranks.push(3);
            match_parts(rest, tail, params, ranks, penalty, steps, limit)
        }
        Segment::Static(expected) => {
            let Some((value, tail)) = values.split_first() else {
                return Ok(None);
            };
            if value != expected {
                return Ok(None);
            }
            ranks.push(4);
            match_parts(rest, tail, params, ranks, penalty, steps, limit)
        }
    }
}
