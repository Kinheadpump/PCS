//! Tools (R-008). A cut job needs an axe and a mine job needs a pick. A colonist without the
//! tool walks to a free tool item, takes one unit, and keeps it in the hand. Each finished
//! job lowers the durability by 1. A new tool has `hardness × 10` durability. At 0 it breaks.

use crate::Sim;
use crate::colonist::{ColonistId, Tool};
use crate::item::{ItemId, ItemLoc};
use crate::job::{Job, JobKind, octile};
use base::content::ToolKind;

/// A tool item that a colonist can walk to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ToolSource {
    pub item: ItemId,
    pub at: (u16, u16),
}

impl Sim {
    /// The tool that a job needs. A ditch needs none.
    pub(crate) fn tool_needed(job: &Job) -> Option<ToolKind> {
        match job.kind {
            JobKind::Cut => Some(ToolKind::Axe),
            JobKind::Mine if !job.is_dig() => Some(ToolKind::Pick),
            _ => None,
        }
    }

    /// The free tool item of a kind that is nearest to a tile in a region. A tie goes to
    /// the lowest item ID.
    pub(crate) fn find_tool(
        &self,
        kind: ToolKind,
        region: u32,
        from: (u16, u16),
    ) -> Option<ToolSource> {
        let mut best: Option<((u32, u32), ToolSource)> = None;
        for (id, item) in self.items.iter() {
            let ItemLoc::Tile(x, y) = item.loc else {
                continue;
            };
            if item.reserved_by.is_some()
                || self.rules.tool_kind(item.material) != Some(kind)
                || self.regions.id_at(i32::from(x), i32::from(y)) != region
            {
                continue;
            }
            let key = (octile(from, (x, y)), id.index());
            if best.is_none_or(|(k, _)| key < k) {
                best = Some((
                    key,
                    ToolSource {
                        item: id,
                        at: (x, y),
                    },
                ));
            }
        }
        best.map(|(_, s)| s)
    }

    /// The colonist takes one unit of a reserved tool item into the hand.
    pub(crate) fn take_tool(&mut self, id: ColonistId, item: ItemId) -> bool {
        let Some(material) = self.items.get(item).map(|i| i.material) else {
            return false;
        };
        let Some(kind) = self.rules.tool_kind(material) else {
            return false;
        };
        let durability = u16::try_from(self.rules.hardness(material) * 10).unwrap_or(u16::MAX);
        if !self.items.take_one(item) {
            return false;
        }
        if let Some(c) = self.colonists.get_mut(id) {
            c.tools[kind.index()] = Some(Tool {
                material,
                durability,
            });
        }
        true
    }

    /// A finished job wears the tool by 1. A tool at 0 breaks.
    pub(crate) fn wear_tool(&mut self, id: ColonistId, kind: ToolKind) {
        if let Some(c) = self.colonists.get_mut(id)
            && let Some(tool) = c.tools[kind.index()].as_mut()
        {
            tool.durability = tool.durability.saturating_sub(1);
            if tool.durability == 0 {
                c.tools[kind.index()] = None;
            }
        }
    }

    /// The colonist puts the tools on the floor (a death).
    pub(crate) fn drop_tools(&mut self, id: ColonistId) {
        let Some(c) = self.colonists.get_mut(id) else {
            return;
        };
        let (x, y) = c.tile();
        let tools = std::mem::take(&mut c.tools);
        for tool in tools.into_iter().flatten() {
            self.spawn_item(tool.material, 1, x, y, true);
        }
    }

    /// Give a colonist a tool with a set durability. For scenario tests and debug tools.
    pub fn debug_set_tool(&mut self, id: ColonistId, material: u16, durability: u16) {
        let Some(kind) = self.rules.tool_kind(material) else {
            return;
        };
        if let Some(c) = self.colonists.get_mut(id) {
            c.tools[kind.index()] = Some(Tool {
                material,
                durability,
            });
        }
    }

    /// Take all tools from a colonist. For scenario tests and debug tools.
    pub fn debug_clear_tools(&mut self, id: ColonistId) {
        if let Some(c) = self.colonists.get_mut(id) {
            c.tools = [None; ToolKind::COUNT];
        }
    }
}
