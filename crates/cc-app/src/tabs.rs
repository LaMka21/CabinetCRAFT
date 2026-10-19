//! Záložky: pôdorys, pohľad na stenu, 3D náhľad (Výroba a Kontrola sú v `production.rs`,
//! Výkresy v `drawings.rs`, Zostavy v `reports.rs`).

use cc_i18n::tr;
use cc_model::Role;
use eframe::egui;
use serde_json::json;

use crate::app::App;

impl App {
    /// Pôdorys a pohľad na stenu menia kópiu projektu; zmena ide ako príkazy `wall.*`,
    /// `opening.*`, `room.set` a `cabinet.set` (`cc_engine::room_edit_commands`), čo sa nimi
    /// vyjadriť nedá, ako `project.set`.
    fn room_view(&mut self, ui: &mut egui::Ui, elevation: bool) {
        let Some(doc) = self.session.doc() else {
            return;
        };
        let mut project = doc.project.clone();
        let before = self.selected_index();
        let mut selected = before;
        let changed = if elevation {
            let prefer = self.plan.selected_wall.clone();
            let built = self.session.model();
            crate::wall_view::show(
                ui,
                &mut self.wall_view,
                &mut project,
                built.as_ref().map(|b| &b.model),
                &mut selected,
                prefer.as_deref(),
            )
        } else {
            crate::plan_view::show(ui, &mut self.plan, &mut project, &mut selected)
        };
        if changed {
            let before = self.session.doc().map(|d| d.project.clone());
            let cmds = before
                .as_ref()
                .and_then(|b| cc_engine::room_edit_commands(b, &project));
            if let Some(cmds) = cmds {
                for (id, mut params) in cmds {
                    params["coalesce"] = json!(true);
                    if self.exec(id, params).is_none() {
                        break;
                    }
                }
            }
            // poistka: výsledok musí byť presne kópia z pohľadu
            if self.session.doc().is_some_and(|d| d.project != project) {
                self.exec(
                    "project.set",
                    json!({ "project": project, "coalesce": true }),
                );
            }
        }
        // položka katalógu pustená myšou na stenu
        let dropped = if elevation {
            self.wall_view.dropped.take()
        } else {
            self.plan.dropped.take()
        };
        if let Some(params) = dropped {
            self.exec("cabinet.add", params);
        }
        if let Some(id) = self.wall_view.props.take() {
            self.open_props(&id);
        }
        // dvojklik na skrinku v pôdoryse alebo pohľade: editor skrinky
        let open = self
            .wall_view
            .open_cabinet
            .take()
            .or_else(|| self.plan.open_cabinet.take());
        if let Some(id) = open {
            self.open_cabinet_editor(&id);
        }
        // príkazy editora sekcií z pohľadu na stenu a ponuky rohu v pôdoryse
        let mut cmds = std::mem::take(&mut self.wall_view.commands);
        cmds.extend(std::mem::take(&mut self.plan.commands));
        for (id, params) in cmds {
            self.exec(id, params);
        }
        if selected != before {
            self.select_index(selected);
        }
    }

    pub(crate) fn tab_plan(&mut self, ui: &mut egui::Ui) {
        self.room_view(ui, false);
    }

    pub(crate) fn tab_elevation(&mut self, ui: &mut egui::Ui) {
        self.room_view(ui, true);
    }
}

/// Názov úlohy dielu (malými písmenami) v zvolenom jazyku.
pub fn role_name(role: Role) -> String {
    match role {
        Role::SideLeft => tr!("tabs.role.side_left"),
        Role::SideRight => tr!("tabs.role.side_right"),
        Role::Bottom => tr!("tabs.role.bottom"),
        Role::Top => tr!("tabs.role.top"),
        Role::StretcherFront => tr!("tabs.role.stretcher_front"),
        Role::StretcherRear => tr!("tabs.role.stretcher_rear"),
        Role::Back => tr!("tabs.role.back"),
        Role::AdjustableShelf => tr!("tabs.role.adjustable_shelf"),
        Role::FixedShelf => tr!("tabs.role.fixed_shelf"),
        Role::Door => tr!("tabs.role.door"),
        Role::DrawerFront => tr!("tabs.role.drawer_front"),
        Role::FalseFront => tr!("tabs.role.false_front"),
        Role::DrawerBottom => tr!("tabs.role.drawer_bottom"),
        Role::DrawerBack => tr!("tabs.role.drawer_back"),
        Role::Plinth => tr!("tabs.role.plinth"),
        Role::FillerStrip => tr!("tabs.role.filler_strip"),
        Role::FillerFront => tr!("tabs.role.filler_front"),
        Role::FillerBottom => tr!("tabs.role.filler_bottom"),
        Role::FinishedBack => tr!("tabs.role.finished_back"),
        Role::LedRail => tr!("tabs.role.led_rail"),
        Role::CoverSide => tr!("tabs.role.cover_side"),
    }
}
