use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use crate::look::clamp_window_size;
use super::{HostKind, Layout, UiState};
use mt1::ui::Preset;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutPreset { #[default] Modern, Classic }
impl From<LayoutPreset> for Preset { fn from(p: LayoutPreset) -> Self { match p { LayoutPreset::Modern => Self::Modern, LayoutPreset::Classic => Self::Classic } } }
impl From<Preset> for LayoutPreset { fn from(p: Preset) -> Self { match p { Preset::Modern => Self::Modern, Preset::Classic => Self::Classic } } }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ComponentSettings {
    pub host: HostKind,
    pub window_pos: Option<[i32; 2]>,
    pub window_size: Option<[u32; 2]>,
    pub size: Option<[u32; 2]>,
}
impl Default for ComponentSettings {
    fn default() -> Self { Self { host: HostKind::Docked, window_pos: None, window_size: None, size: None } }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutSettings {
    pub preset: LayoutPreset,
    pub log_lines: u8,
    pub components: BTreeMap<String, ComponentSettings>,
}
impl Default for LayoutSettings {
    fn default() -> Self { Self { preset: LayoutPreset::Modern, log_lines: 3, components: BTreeMap::new() } }
}
impl LayoutSettings {
    pub fn clamp(&mut self) {
        self.log_lines = self.log_lines.clamp(2, 20);
        for saved in self.components.values_mut() {
            saved.window_size = saved.window_size.filter(|v| !v.contains(&0)).map(clamp_window_size);
            saved.size = saved.size.filter(|v| !v.contains(&0)).map(clamp_window_size);
        }
    }
    pub fn restore(&mut self, layout: &Layout, ui: &mut UiState) {
        self.clamp();
        self.components.retain(|id, _| layout.ids().iter().any(|known| *known == id));
        ui.preset = self.preset.into(); ui.log_lines = self.log_lines;
        ui.hosts.clear();
        for id in layout.ids() {
            if let Some(saved) = self.components.get_mut(id) {
                if let Some(p) = layout.placement(id) {
                    if saved.host == HostKind::Window && !p.undockable || saved.host == HostKind::Hidden && !p.optional { saved.host = HostKind::Docked; }
                }
                ui.hosts.push((id, saved.host));
            }
        }
    }
}

pub fn visible_position(position: Option<[i32; 2]>, monitors: impl IntoIterator<Item = ([i32; 2], [u32; 2])>) -> Option<[i32; 2]> {
    let [x, y] = position?;
    monitors.into_iter().any(|([mx, my], [w, h])| {
        i64::from(x) >= i64::from(mx) && i64::from(y) >= i64::from(my)
            && i64::from(x) < i64::from(mx) + i64::from(w) && i64::from(y) < i64::from(my) + i64::from(h)
    }).then_some([x, y])
}
