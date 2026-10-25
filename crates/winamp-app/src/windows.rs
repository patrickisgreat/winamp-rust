//! Window geometry: tracking the three skinned windows, dragging with
//! snapping to screen edges and to each other, and docked group moves.

use egui::{Pos2, Rect, Vec2, ViewportCommand, ViewportId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Win {
    Main,
    Eq,
    Pl,
}

impl Win {
    pub const ALL: [Win; 3] = [Win::Main, Win::Eq, Win::Pl];

    pub fn viewport_id(self) -> ViewportId {
        match self {
            Win::Main => ViewportId::ROOT,
            Win::Eq => ViewportId::from_hash_of("winamp-eq"),
            Win::Pl => ViewportId::from_hash_of("winamp-pl"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WinInfo {
    /// Last known outer rect in screen points.
    pub rect: Option<Rect>,
    pub visible: bool,
    pub shaded: bool,
    pub focused: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Drag {
    pub win: Win,
    /// Pointer position (window points) when the drag started.
    pub grab: Pos2,
    /// Windows that move together with the dragged one.
    pub group: [bool; 3],
}

pub struct Windows {
    pub info: [WinInfo; 3],
    pub drag: Option<Drag>,
    pub monitor: Option<Vec2>,
    pub snap: bool,
}

const SNAP: f32 = 10.0;

impl Windows {
    pub fn new() -> Self {
        Self { info: [WinInfo::default(); 3], drag: None, monitor: None, snap: true }
    }

    pub fn get(&self, w: Win) -> &WinInfo {
        &self.info[idx(w)]
    }
    pub fn get_mut(&mut self, w: Win) -> &mut WinInfo {
        &mut self.info[idx(w)]
    }

    /// Record the rect reported by egui for a viewport this frame.
    pub fn observe(&mut self, w: Win, ctx: &egui::Context) {
        let (rect, focused, monitor) = ctx.input(|i| {
            let v = i.viewport();
            (v.outer_rect.or(v.inner_rect), v.focused.unwrap_or(false), v.monitor_size)
        });
        let not_dragging_me = self.drag.map(|d| d.win != w).unwrap_or(true);
        let info = self.get_mut(w);
        if let Some(r) = rect {
            if not_dragging_me || info.rect.is_none() {
                // While dragging, trust our own position to avoid feedback jitter.
                info.rect = Some(r);
            }
        }
        info.focused = focused;
        if monitor.is_some() {
            self.monitor = monitor;
        }
    }

    pub fn begin_drag(&mut self, w: Win, grab: Pos2) {
        let group = self.docked_group(w);
        self.drag = Some(Drag { win: w, grab, group });
    }

    pub fn end_drag(&mut self) {
        self.drag = None;
    }

    /// Continue a drag with the current pointer (window points). Sends move commands.
    pub fn update_drag(&mut self, ctx: &egui::Context, pointer: Pos2, allow_snap: bool) {
        let Some(drag) = self.drag else { return };
        let Some(rect) = self.get(drag.win).rect else { return };
        let delta = pointer - drag.grab;
        if delta == Vec2::ZERO {
            return;
        }
        let mut target = rect.translate(delta);
        if allow_snap && self.snap {
            target = self.snap_rect(drag.win, target, &drag.group);
        }
        let actual_delta = target.min - rect.min;
        for w in Win::ALL {
            if w == drag.win || drag.group[idx(w)] {
                if let Some(r) = self.get(w).rect {
                    let nr = r.translate(actual_delta);
                    self.get_mut(w).rect = Some(nr);
                    ctx.send_viewport_cmd_to(w.viewport_id(), ViewportCommand::OuterPosition(nr.min));
                }
            }
        }
    }

    fn snap_rect(&self, me: Win, mut r: Rect, group: &[bool; 3]) -> Rect {
        let mut dx: Option<f32> = None;
        let mut dy: Option<f32> = None;
        let mut consider_x = |cand: f32| {
            if cand.abs() < SNAP && dx.map(|d| cand.abs() < d.abs()).unwrap_or(true) {
                dx = Some(cand);
            }
        };
        let mut consider_y = |cand: f32| {
            if cand.abs() < SNAP && dy.map(|d| cand.abs() < d.abs()).unwrap_or(true) {
                dy = Some(cand);
            }
        };
        if let Some(m) = self.monitor {
            consider_x(0.0 - r.min.x);
            consider_x(m.x - r.max.x);
            consider_y(0.0 - r.min.y);
            consider_y(m.y - r.max.y);
        }
        for w in Win::ALL {
            if w == me || group[idx(w)] {
                continue;
            }
            let o = self.get(w);
            if !o.visible {
                continue;
            }
            let Some(or) = o.rect else { continue };
            // Horizontal adjacency (side by side) requires vertical overlap, and vice versa.
            let v_overlap = r.min.y < or.max.y + SNAP && r.max.y > or.min.y - SNAP;
            let h_overlap = r.min.x < or.max.x + SNAP && r.max.x > or.min.x - SNAP;
            if v_overlap {
                consider_x(or.max.x - r.min.x); // my left to their right
                consider_x(or.min.x - r.max.x); // my right to their left
                consider_x(or.min.x - r.min.x); // align lefts
                consider_x(or.max.x - r.max.x); // align rights
            }
            if h_overlap {
                consider_y(or.max.y - r.min.y);
                consider_y(or.min.y - r.max.y);
                consider_y(or.min.y - r.min.y);
                consider_y(or.max.y - r.max.y);
            }
        }
        if let Some(d) = dx {
            r = r.translate(Vec2::new(d, 0.0));
        }
        if let Some(d) = dy {
            r = r.translate(Vec2::new(0.0, d));
        }
        r
    }

    /// Windows that are touching `root` (directly or transitively) move with it.
    fn docked_group(&self, root: Win) -> [bool; 3] {
        let mut group = [false; 3];
        if root != Win::Main {
            // Only the main window drags its docked companions (as in Winamp).
            return group;
        }
        let mut changed = true;
        group[idx(root)] = true;
        while changed {
            changed = false;
            for w in Win::ALL {
                if group[idx(w)] || !self.get(w).visible {
                    continue;
                }
                let Some(r) = self.get(w).rect else { continue };
                for other in Win::ALL {
                    if !group[idx(other)] {
                        continue;
                    }
                    let Some(or) = self.get(other).rect else { continue };
                    if touching(r, or) {
                        group[idx(w)] = true;
                        changed = true;
                        break;
                    }
                }
            }
        }
        group[idx(root)] = false;
        group
    }

    /// Default layout: EQ below main, playlist below EQ.
    pub fn default_position(&self, w: Win) -> Pos2 {
        let main = self.get(Win::Main).rect;
        match (w, main) {
            (Win::Main, _) => Pos2::new(120.0, 80.0),
            (Win::Eq, Some(m)) => Pos2::new(m.min.x, m.max.y),
            (Win::Pl, Some(m)) => {
                let eq = self.get(Win::Eq);
                if eq.visible {
                    if let Some(e) = eq.rect {
                        return Pos2::new(m.min.x, e.max.y);
                    }
                }
                Pos2::new(m.min.x, m.max.y)
            }
            (_, None) => Pos2::new(120.0, 80.0),
        }
    }
}

fn idx(w: Win) -> usize {
    match w {
        Win::Main => 0,
        Win::Eq => 1,
        Win::Pl => 2,
    }
}

fn touching(a: Rect, b: Rect) -> bool {
    let eps = 2.0;
    let v_overlap = a.min.y < b.max.y + eps && a.max.y > b.min.y - eps;
    let h_overlap = a.min.x < b.max.x + eps && a.max.x > b.min.x - eps;
    let side = ((a.min.x - b.max.x).abs() < eps || (a.max.x - b.min.x).abs() < eps) && v_overlap;
    let stacked = ((a.min.y - b.max.y).abs() < eps || (a.max.y - b.min.y).abs() < eps) && h_overlap;
    side || stacked
}
