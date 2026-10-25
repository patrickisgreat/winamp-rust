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
    /// Offset of each companion window's top-left from the dragged window's at drag start.
    pub offsets: [Vec2; 3],
    /// Last position sent to each window, to avoid flooding identical commands.
    pub last_sent: [Option<Pos2>; 3],
}

pub struct Windows {
    pub info: [WinInfo; 3],
    pub drag: Option<Drag>,
    pub monitor: Option<Vec2>,
    pub snap: bool,
    /// When set, every visible window moves together regardless of adjacency.
    pub lock: bool,
}

const SNAP: f32 = 10.0;

impl Windows {
    pub fn new() -> Self {
        Self {
            info: [WinInfo::default(); 3],
            drag: None,
            monitor: None,
            snap: true,
            lock: true,
        }
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
            (
                v.outer_rect.or(v.inner_rect),
                v.focused.unwrap_or(false),
                v.monitor_size,
            )
        });
        let info = self.get_mut(w);
        if let Some(r) = rect {
            // Always trust the OS: the pointer position we get is relative to this rect.
            info.rect = Some(r);
        }
        info.focused = focused;
        if monitor.is_some() {
            self.monitor = monitor;
        }
    }

    pub fn begin_drag(&mut self, w: Win, grab: Pos2) {
        let group = self.docked_group(w);
        let base = self.get(w).rect.map(|r| r.min).unwrap_or(Pos2::ZERO);
        let mut offsets = [Vec2::ZERO; 3];
        for o in Win::ALL {
            if let Some(r) = self.get(o).rect {
                offsets[idx(o)] = r.min - base;
            }
        }
        self.drag = Some(Drag {
            win: w,
            grab,
            group,
            offsets,
            last_sent: [None; 3],
        });
    }

    pub fn end_drag(&mut self) {
        self.drag = None;
    }

    /// Continue a drag with the current pointer (window points). Sends move commands.
    pub fn update_drag(&mut self, ctx: &egui::Context, pointer: Pos2, allow_snap: bool) {
        let Some(mut drag) = self.drag else { return };
        let Some(rect) = self.get(drag.win).rect else {
            return;
        };
        // The pointer is relative to the window as the OS currently has it, so
        // the desired top-left is simply where the pointer is minus the grab offset.
        let delta = pointer - drag.grab;
        let mut target = rect.translate(delta);
        if allow_snap && self.snap {
            target = self.snap_rect(drag.win, target, &drag.group);
        }
        for w in Win::ALL {
            if w != drag.win && !drag.group[idx(w)] {
                continue;
            }
            let Some(r) = self.get(w).rect else { continue };
            let pos = if w == drag.win {
                target.min
            } else {
                target.min + drag.offsets[idx(w)]
            };
            let pos = Pos2::new(pos.x.round(), pos.y.round());
            let unchanged = drag.last_sent[idx(w)]
                .map(|p| (p - pos).length() < 0.5)
                .unwrap_or(false);
            if unchanged || (r.min - pos).length() < 0.5 {
                continue;
            }
            drag.last_sent[idx(w)] = Some(pos);
            if w != drag.win {
                // Companions are not observed until their own frame; keep our copy current.
                self.get_mut(w).rect = Some(Rect::from_min_size(pos, r.size()));
            }
            ctx.send_viewport_cmd_to(w.viewport_id(), ViewportCommand::OuterPosition(pos));
        }
        self.drag = Some(drag);
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
    pub fn docked_group(&self, root: Win) -> [bool; 3] {
        let mut group = [false; 3];
        if self.lock {
            for w in Win::ALL {
                group[idx(w)] = w != root && self.get(w).visible;
            }
            return group;
        }
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
                    let Some(or) = self.get(other).rect else {
                        continue;
                    };
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

    /// Re-stack the windows docked to the main window (EQ under main, playlist
    /// under EQ) after a size change such as double size or windowshade.
    pub fn restack(
        &mut self,
        ctx: &egui::Context,
        group: [bool; 3],
        main_h: f32,
        eq_h: f32,
        scale: f32,
    ) {
        let Some(main) = self.get(Win::Main).rect else {
            return;
        };
        let mut y = main.min.y + main_h * scale;
        let x = main.min.x;
        for w in [Win::Eq, Win::Pl] {
            let info = *self.get(w);
            if !group[idx(w)] || !info.visible {
                continue;
            }
            if let Some(r) = info.rect {
                let nr = Rect::from_min_size(Pos2::new(x, y), r.size());
                self.get_mut(w).rect = Some(nr);
                ctx.send_viewport_cmd_to(w.viewport_id(), ViewportCommand::OuterPosition(nr.min));
                y += if w == Win::Eq {
                    eq_h * scale
                } else {
                    r.height()
                };
            }
        }
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
    let eps = 6.0;
    let v_overlap = a.min.y < b.max.y + eps && a.max.y > b.min.y - eps;
    let h_overlap = a.min.x < b.max.x + eps && a.max.x > b.min.x - eps;
    let side = ((a.min.x - b.max.x).abs() < eps || (a.max.x - b.min.x).abs() < eps) && v_overlap;
    let stacked = ((a.min.y - b.max.y).abs() < eps || (a.max.y - b.min.y).abs() < eps) && h_overlap;
    side || stacked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h))
    }

    fn windows_with(main: Rect, eq: Option<Rect>, pl: Option<Rect>) -> Windows {
        let mut windows = Windows::new();
        windows.lock = false;
        windows.get_mut(Win::Main).rect = Some(main);
        windows.get_mut(Win::Main).visible = true;
        windows.get_mut(Win::Eq).rect = eq;
        windows.get_mut(Win::Eq).visible = eq.is_some();
        windows.get_mut(Win::Pl).rect = pl;
        windows.get_mut(Win::Pl).visible = pl.is_some();
        windows
    }

    #[test]
    fn touching_detects_stacked_and_side_by_side_windows() {
        let main = rect(0.0, 0.0, 275.0, 116.0);
        assert!(
            touching(main, rect(0.0, 116.0, 275.0, 116.0)),
            "directly below"
        );
        assert!(
            touching(main, rect(275.0, 20.0, 275.0, 116.0)),
            "directly right"
        );
        assert!(
            touching(main, rect(3.0, 119.0, 275.0, 116.0)),
            "within the tolerance"
        );
        assert!(!touching(main, rect(0.0, 140.0, 275.0, 116.0)), "gap below");
        assert!(
            !touching(main, rect(275.0, 200.0, 275.0, 116.0)),
            "right but no vertical overlap"
        );
    }

    #[test]
    fn docked_group_follows_transitive_contact_from_main_only() {
        let main = rect(0.0, 0.0, 275.0, 116.0);
        let eq = rect(0.0, 116.0, 275.0, 116.0);
        let pl = rect(0.0, 232.0, 275.0, 116.0);
        let windows = windows_with(main, Some(eq), Some(pl));
        assert_eq!(
            windows.docked_group(Win::Main),
            [false, true, true],
            "playlist touches via the EQ"
        );
        assert_eq!(
            windows.docked_group(Win::Eq),
            [false, false, false],
            "only the main window drags companions"
        );

        let far_pl = rect(600.0, 600.0, 275.0, 116.0);
        let windows = windows_with(main, Some(eq), Some(far_pl));
        assert_eq!(windows.docked_group(Win::Main), [false, true, false]);

        let mut hidden_eq = windows_with(main, Some(eq), Some(pl));
        hidden_eq.get_mut(Win::Eq).visible = false;
        assert_eq!(
            hidden_eq.docked_group(Win::Main),
            [false, false, false],
            "a hidden EQ breaks the chain"
        );
    }

    #[test]
    fn locked_windows_always_move_together() {
        let mut windows = windows_with(
            rect(0.0, 0.0, 275.0, 116.0),
            Some(rect(900.0, 0.0, 275.0, 116.0)),
            None,
        );
        windows.lock = true;
        assert_eq!(windows.docked_group(Win::Main), [false, true, false]);
        assert_eq!(
            windows.docked_group(Win::Eq),
            [true, false, false],
            "any window drags the others when locked"
        );
    }

    #[test]
    fn snapping_pulls_windows_onto_screen_edges_and_neighbours() {
        let mut windows = windows_with(
            rect(100.0, 100.0, 275.0, 116.0),
            Some(rect(400.0, 100.0, 275.0, 116.0)),
            None,
        );
        windows.monitor = Some(Vec2::new(1000.0, 800.0));
        let group = [false; 3];
        // 6 px from the left screen edge: snaps to 0.
        let snapped = windows.snap_rect(Win::Pl, rect(6.0, 300.0, 275.0, 116.0), &group);
        assert_eq!(snapped.min, Pos2::new(0.0, 300.0));
        // Right edge 4 px short of the EQ's left edge, vertically overlapping: snaps flush.
        let snapped = windows.snap_rect(Win::Pl, rect(121.0, 110.0, 275.0, 116.0), &group);
        assert_eq!(snapped.min.x, 400.0 - 275.0);
        // Far from everything: unchanged.
        let free = rect(500.0, 500.0, 275.0, 116.0);
        assert_eq!(windows.snap_rect(Win::Pl, free, &group), free);
        // Snapping is skipped against windows in the dragged group.
        let snapped = windows.snap_rect(
            Win::Pl,
            rect(121.0, 110.0, 275.0, 116.0),
            &[false, true, false],
        );
        assert_eq!(snapped.min.x, 121.0);
    }

    #[test]
    fn default_positions_stack_under_the_main_window() {
        let main = rect(50.0, 60.0, 275.0, 116.0);
        let windows = windows_with(main, Some(rect(50.0, 176.0, 275.0, 116.0)), None);
        assert_eq!(windows.default_position(Win::Eq), Pos2::new(50.0, 176.0));
        assert_eq!(
            windows.default_position(Win::Pl),
            Pos2::new(50.0, 292.0),
            "below the visible EQ"
        );
        let no_eq = windows_with(main, None, None);
        assert_eq!(no_eq.default_position(Win::Pl), Pos2::new(50.0, 176.0));
        assert_eq!(
            Windows::new().default_position(Win::Eq),
            Pos2::new(120.0, 80.0),
            "no main rect known yet"
        );
    }
}
