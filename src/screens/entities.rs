use crate::config::Alias;
use crate::ha::{EntityId, EntityState, InstanceRuntime};

/// One entry in the rendered entity list.
pub struct EntityRow<'a> {
    pub instance: &'a Alias,
    pub state: &'a EntityState,
}

/// Flatten all instances' entities into a single sorted, optionally-filtered list.
pub fn build_rows<'a>(
    runtimes: impl Iterator<Item = &'a InstanceRuntime>,
    instance_filter: Option<&Alias>,
    search: &str,
) -> Vec<EntityRow<'a>> {
    let mut rows: Vec<EntityRow<'a>> = runtimes
        .filter(|r| instance_filter.map(|f| &r.alias == f).unwrap_or(true))
        .flat_map(|r| {
            r.states.values().map(move |s| EntityRow {
                instance: &r.alias,
                state: s,
            })
        })
        .filter(|row| search.is_empty() || row.state.entity_id.contains(search))
        .collect();
    rows.sort_by(|a, b| a.state.entity_id.cmp(&b.state.entity_id));
    rows
}

#[allow(dead_code)]
pub fn entity_domain(id: &EntityId) -> &str {
    id.split_once('.').map(|(d, _)| d).unwrap_or(id)
}
