//! Pás nástrojov (ribbon): karty Súbor / Hlavné / Nástroje, skupiny tlačidiel
//! podľa aktuálneho pohľadu a lišta pohľadov nad pracovnou plochou.
//!
//! Rozloženie zodpovedá bežným CAD programom (aj CABINET VISION), ikony a texty sú vlastné.

use cc_i18n::tr;
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, Sense, Stroke};
use serde_json::json;

use crate::app::{App, Pending, Tab};
use crate::icons::{self, Icon};
use crate::plan_modes::PlanMode;
use crate::plan_view::Tool;
use crate::theme;

/// Výška obsahu skupiny (bez popisu skupiny).
const GROUP_H: f32 = 70.0;
const BIG_W: f32 = 54.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum RibbonTab {
    #[default]
    Main,
    Tools,
}

/// Stav rozloženia okna, ktorý nepatrí projektu.
#[derive(Debug, Clone)]
pub(crate) struct Layout {
    pub ribbon: RibbonTab,
    pub file_menu: bool,
    /// Rozbalené skupiny vlastností v ľavom paneli (podľa mena).
    pub collapsed: std::collections::BTreeSet<String>,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            ribbon: RibbonTab::Main,
            file_menu: false,
            collapsed: Default::default(),
        }
    }
}

/// Veľké tlačidlo: ikona 32 bodov a text pod ňou (najviac dva riadky).
pub(crate) fn big_button(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    enabled: bool,
    active: bool,
) -> egui::Response {
    let galley = ui.painter().layout(
        two_lines(label),
        FontId::proportional(theme::BODY - 0.5),
        theme::TEXT,
        BIG_W + 16.0,
    );
    let width = BIG_W.max(galley.size().x + 8.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, GROUP_H), sense);
    paint_button_bg(ui, rect, &response, enabled, active);
    let icon_rect =
        Rect::from_center_size(pos2(rect.center().x, rect.min.y + 18.0), vec2(30.0, 30.0));
    icons::paint(ui.painter(), icon_rect, icon, enabled);
    let color = if enabled {
        theme::TEXT
    } else {
        theme::TEXT_WEAK
    };
    let galley = ui.painter().layout(
        two_lines(label),
        FontId::proportional(theme::BODY - 0.5),
        color,
        width,
    );
    let text_pos = pos2(rect.center().x - galley.size().x / 2.0, rect.min.y + 36.0);
    ui.painter().galley(text_pos, galley, color);
    response
}

/// Text tlačidla na najviac dva riadky (zlom pri medzere najbližšie k stredu).
fn two_lines(label: &str) -> String {
    let mid = label.chars().count() / 2;
    let best = label
        .char_indices()
        .filter(|(_, c)| *c == ' ')
        .map(|(i, _)| i)
        .min_by_key(|&i| (label[..i].chars().count() as i64 - mid as i64).abs());
    match best {
        Some(i) => format!("{}\n{}", &label[..i], &label[i + 1..]),
        None => label.to_owned(),
    }
}

/// Malé tlačidlo: ikona 16 bodov a text vedľa (do stĺpca po troch).
pub(crate) fn small_button(
    ui: &mut egui::Ui,
    icon: Option<Icon>,
    label: &str,
    enabled: bool,
    active: bool,
) -> egui::Response {
    let font = FontId::proportional(theme::BODY - 0.5);
    let color = if enabled {
        theme::TEXT
    } else {
        theme::TEXT_WEAK
    };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, color);
    let width = galley.size().x + if icon.is_some() { 26.0 } else { 8.0 };
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, 20.0), sense);
    paint_button_bg(ui, rect, &response, enabled, active);
    let mut x = rect.min.x + 4.0;
    if let Some(icon) = icon {
        icons::paint(
            ui.painter(),
            Rect::from_min_size(pos2(x, rect.min.y + 2.0), vec2(16.0, 16.0)),
            icon,
            enabled,
        );
        x += 20.0;
    }
    ui.painter().galley(
        pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    response
}

/// Zaškrtávacie políčko v páse nástrojov.
fn small_check(ui: &mut egui::Ui, value: &mut bool, label: &str) -> bool {
    ui.checkbox(value, label).changed()
}

fn paint_button_bg(
    ui: &egui::Ui,
    rect: Rect,
    response: &egui::Response,
    enabled: bool,
    active: bool,
) {
    let (fill, stroke) = if active {
        (theme::ACTIVE_BG, Stroke::new(1.0_f32, theme::BORDER))
    } else if enabled && response.is_pointer_button_down_on() {
        (theme::SELECT_BG, Stroke::new(1.0_f32, theme::SELECT_STROKE))
    } else if enabled && response.hovered() {
        (theme::HOVER_BG, Stroke::new(1.0_f32, theme::HOVER_STROKE))
    } else {
        return;
    };
    ui.painter().rect_filled(rect, 2.0, fill);
    ui.painter()
        .rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
}

/// Skupina pása: obsah vo výške [`GROUP_H`], pod ním popis a vpravo oddeľovač.
pub(crate) fn group(ui: &mut egui::Ui, caption: &str, add: impl FnOnce(&mut egui::Ui)) {
    let top = ui.cursor().min.y;
    let inner = ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.set_min_height(GROUP_H);
            ui.set_max_height(GROUP_H);
            add(ui);
        });
        ui.add_space(2.0);
    });
    let r = inner.response.rect;
    let width = r.width().max(40.0);
    ui.painter().text(
        pos2(r.min.x + width / 2.0, top + GROUP_H + 9.0),
        Align2::CENTER_CENTER,
        caption,
        FontId::proportional(theme::SMALL),
        theme::TEXT_WEAK,
    );
    ui.add_space(4.0);
    let x = ui.cursor().min.x;
    ui.painter().line_segment(
        [pos2(x, top + 2.0), pos2(x, top + GROUP_H + 16.0)],
        Stroke::new(1.0_f32, theme::BORDER),
    );
    ui.add_space(5.0);
}

/// Stĺpec malých tlačidiel v skupine.
pub(crate) fn column(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.set_min_height(GROUP_H);
        add(ui);
    });
}

impl App {
    /// Riadok s kartami (Súbor, Hlavné, Nástroje) a pás nástrojov.
    pub(crate) fn ribbon(&mut self, ui: &mut egui::Ui) {
        self.ribbon_tabs(ui);
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.add_space(4.0);
            match self.layout.ribbon {
                RibbonTab::Main => self.ribbon_main(ui),
                RibbonTab::Tools => self.ribbon_tools(ui),
            }
        });
        ui.add_space(4.0);
    }

    fn ribbon_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(8.0);
            let file = ui.add(
                egui::Label::new(
                    egui::RichText::new(tr!("ribbon.tabs.file"))
                        .size(theme::BODY)
                        .color(theme::TEXT),
                )
                .sense(Sense::click()),
            );
            if file.clicked() {
                self.layout.file_menu = !self.layout.file_menu;
            }
            let file_rect = file.rect;
            for (tab, label) in [
                (RibbonTab::Main, tr!("ribbon.tabs.main")),
                (RibbonTab::Tools, tr!("ribbon.tabs.tools")),
            ] {
                ui.add_space(14.0);
                let active = self.layout.ribbon == tab;
                let text = egui::RichText::new(label).size(theme::BODY);
                let text = if active {
                    text.strong().color(theme::TEXT)
                } else {
                    text.color(theme::TEXT)
                };
                let r = ui.add(egui::Label::new(text).sense(Sense::click()));
                if active {
                    ui.painter().line_segment(
                        [
                            pos2(r.rect.min.x - 2.0, r.rect.max.y + 2.0),
                            pos2(r.rect.max.x + 2.0, r.rect.max.y + 2.0),
                        ],
                        Stroke::new(2.0_f32, theme::SELECT_STROKE),
                    );
                }
                if r.clicked() {
                    self.layout.ribbon = tab;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if let Some(doc) = self.session.doc() {
                    ui.label(
                        egui::RichText::new(doc.title())
                            .size(theme::SMALL)
                            .color(theme::TEXT_WEAK),
                    );
                }
            });
            if self.layout.file_menu {
                self.file_menu(ui.ctx(), file_rect);
            }
        });
    }

    fn file_menu(&mut self, ctx: &egui::Context, anchor: Rect) {
        let has_doc = self.session.doc().is_some();
        let mut close = false;
        let area = egui::Area::new(egui::Id::new("file_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos2(anchor.min.x - 4.0, anchor.max.y + 4.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(200.0);
                    let can_print = self.session.model().is_some();
                    let can_cvj = self
                        .session
                        .doc()
                        .is_some_and(|d| d.project.project.cv_template.is_some());
                    let items: [(Icon, String, &str, bool); 8] = [
                        (Icon::New, tr!("ribbon.file.new"), "Ctrl+N", has_doc),
                        (Icon::Open, tr!("ribbon.file.open"), "Ctrl+O", true),
                        (Icon::Save, tr!("ribbon.file.save"), "Ctrl+S", has_doc),
                        (Icon::SaveAs, tr!("ribbon.file.save_as"), "", has_doc),
                        (Icon::Materials, tr!("ribbon.file.import_cvj"), "", true),
                        (Icon::SaveAs, tr!("ribbon.file.export_cvj"), "", can_cvj),
                        (Icon::Drawing, tr!("ribbon.file.print"), "Ctrl+P", can_print),
                        (Icon::Delete, tr!("ribbon.file.quit"), "", true),
                    ];
                    for (i, (icon, label, key, enabled)) in items.into_iter().enumerate() {
                        let r = ui.horizontal(|ui| {
                            let r = small_button(ui, Some(icon), &label, enabled, false);
                            if !key.is_empty() {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(key)
                                                .size(theme::SMALL)
                                                .color(theme::TEXT_WEAK),
                                        )
                                    },
                                );
                            }
                            r
                        });
                        if r.inner.clicked() {
                            close = true;
                            match i {
                                0 => self.request(Pending::New),
                                1 => self.pick_open(),
                                2 => {
                                    self.save();
                                }
                                3 => {
                                    self.save_as();
                                }
                                4 => self.pick_import(),
                                5 => self.pick_export_cvj(),
                                6 => self.print_drawings(None, true),
                                _ => self.request(Pending::Quit),
                            }
                        }
                    }
                });
            });
        let clicked_outside = ctx.input(|i| i.pointer.any_click())
            && !area
                .response
                .rect
                .contains(ctx.input(|i| i.pointer.interact_pos().unwrap_or_default()))
            && !anchor.contains(ctx.input(|i| i.pointer.interact_pos().unwrap_or_default()));
        if close || clicked_outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.layout.file_menu = false;
        }
    }

    fn ribbon_main(&mut self, ui: &mut egui::Ui) {
        let has_doc = self.session.doc().is_some();
        let undo = self.session.undo_label();
        let redo = self.session.redo_label();

        // Návrat: z režimu pôdorysu na zoznam režimov, z editora skrinky späť (ako v CV)
        group(ui, &tr!("ribbon.home.group"), |ui| {
            let can_return = (matches!(self.tab, Tab::Plan | Tab::Elevation)
                && self.plan_mode() != PlanMode::Home)
                || matches!(self.tab, Tab::Cabinet | Tab::PartEdit);
            if big_button(
                ui,
                Icon::Return,
                &tr!("ribbon.home.return"),
                can_return,
                false,
            )
            .on_hover_text(tr!("ribbon.home.return_hint"))
            .clicked()
            {
                if self.tab == Tab::PartEdit {
                    // otvorená doska → späť (do editora skrinky)
                    self.tab = self.part_back.take().unwrap_or(Tab::Cabinet);
                } else if self.tab == Tab::Cabinet {
                    self.tab = if self.cab_return == Tab::Cabinet {
                        Tab::View3d
                    } else {
                        self.cab_return
                    };
                } else {
                    self.set_plan_mode(PlanMode::Home);
                }
            }
        });

        group(ui, &tr!("ribbon.edit.group"), |ui| {
            if big_button(
                ui,
                Icon::Undo,
                &tr!("ribbon.edit.undo"),
                undo.is_some(),
                false,
            )
            .on_hover_text(tr!(
                "ribbon.edit.undo_hint",
                label = undo.as_deref().unwrap_or("–")
            ))
            .clicked()
            {
                self.undo(false);
            }
            if big_button(
                ui,
                Icon::Redo,
                &tr!("ribbon.edit.redo"),
                redo.is_some(),
                false,
            )
            .on_hover_text(tr!(
                "ribbon.edit.redo_hint",
                label = redo.as_deref().unwrap_or("–")
            ))
            .clicked()
            {
                self.undo(true);
            }
        });

        // editor skrinky a otvorená doska: vlastný pás podľa záložky (ako v CV)
        if self.tab == Tab::Cabinet {
            self.ribbon_cabinet(ui);
            return;
        }
        if self.tab == Tab::PartEdit {
            self.ribbon_part(ui);
            return;
        }
        if matches!(self.tab, Tab::Plan | Tab::Elevation) {
            self.group_zoom(ui);
        }
        // režim Steny: pás s nástrojmi kreslenia a úprav stien (ako v CV), nič iné
        if self.tab == Tab::Plan && self.plan_ui.mode == PlanMode::Walls {
            self.group_wall_tools(ui, has_doc);
            return;
        }
        // pôdorys a pohľad pridávajú skrinky v režime Objekty (ľavý panel)
        match self.tab {
            Tab::Reports => self.group_cabinets(ui, has_doc),
            Tab::View3d => self.group_camera(ui),
            _ => {}
        }

        // v 3D je pás plný kamerových volieb; štandard a materiál sú aj v ľavom paneli
        if self.tab != Tab::View3d {
            group(ui, &tr!("ribbon.project.group"), |ui| {
                column(ui, |ui| {
                    ui.horizontal(|ui| {
                        icons::paint(
                            ui.painter(),
                            Rect::from_min_size(ui.cursor().min + vec2(0.0, 2.0), vec2(16.0, 16.0)),
                            Icon::Standard,
                            has_doc,
                        );
                        ui.add_space(20.0);
                        self.standard_combo(ui);
                    });
                    ui.horizontal(|ui| {
                        icons::paint(
                            ui.painter(),
                            Rect::from_min_size(ui.cursor().min + vec2(0.0, 2.0), vec2(16.0, 16.0)),
                            Icon::Materials,
                            has_doc,
                        );
                        ui.add_space(20.0);
                        self.material_combo(ui);
                    });
                });
            });
        }

        if self.tab == Tab::Elevation {
            self.group_wall_pick(ui);
        }

        group(ui, &tr!("ribbon.properties.group"), |ui| {
            if big_button(
                ui,
                Icon::Walls,
                &tr!("ribbon.properties.room"),
                has_doc,
                false,
            )
            .on_hover_text(tr!("ribbon.properties.room_hint"))
            .clicked()
            {
                self.tab = Tab::Plan;
                self.set_plan_mode(PlanMode::Home);
            }
            if big_button(
                ui,
                Icon::Standard,
                &tr!("ribbon.properties.standard"),
                has_doc,
                self.tab == Tab::Standard,
            )
            .on_hover_text(tr!("ribbon.properties.standard_hint"))
            .clicked()
            {
                self.tab = Tab::Standard;
            }
        });

        group(ui, &tr!("ribbon.drawings.group"), |ui| {
            let can_print = self.session.model().is_some();
            if big_button(
                ui,
                Icon::Drawing,
                &tr!("ribbon.drawings.print"),
                can_print,
                false,
            )
            .on_hover_text(tr!("ribbon.drawings.print_hint"))
            .clicked()
            {
                self.print_drawings(None, false);
            }
            if self.tab == Tab::Elevation {
                let wall = self.wall_view.wall.clone();
                if big_button(
                    ui,
                    Icon::Elevation,
                    &tr!("ribbon.drawings.print_wall"),
                    can_print && wall.is_some(),
                    false,
                )
                .on_hover_text(tr!("ribbon.drawings.print_wall_hint"))
                .clicked()
                {
                    if let Some(id) = wall {
                        self.print_drawings(Some(vec![format!("wall:{id}")]), false);
                    }
                }
            }
        });

        group(ui, &tr!("ribbon.production.group"), |ui| {
            let built = self.session.built();
            let errors = match built {
                Some(Ok(b)) => b.report.errors().count(),
                _ => 0,
            };
            let can_export = matches!(built, Some(Ok(b)) if !b.has_errors());
            if big_button(
                ui,
                Icon::Check,
                &tr!("ribbon.production.check"),
                has_doc,
                self.tab == Tab::Issues,
            )
            .on_hover_text(if errors > 0 {
                tr!("ribbon.production.check_errors", count = errors)
            } else {
                tr!("ribbon.production.check_hint")
            })
            .clicked()
            {
                self.tab = Tab::Issues;
            }
            let r = big_button(
                ui,
                Icon::Export,
                &tr!("ribbon.production.export"),
                can_export,
                false,
            )
            .on_hover_text(tr!("ribbon.production.export_hint"));
            let r = if can_export {
                r
            } else {
                r.on_hover_text(tr!("ribbon.production.export_blocked"))
            };
            if r.clicked() && can_export {
                if let Some(dir) = self.default_export_dir() {
                    self.export(dir);
                }
            }
            column(ui, |ui| {
                if small_button(
                    ui,
                    Some(Icon::Open),
                    &tr!("ribbon.production.to_folder"),
                    can_export,
                    false,
                )
                .clicked()
                {
                    let mut dialog = rfd::FileDialog::new();
                    if let Some(dir) = self.default_export_dir() {
                        let _ = std::fs::create_dir_all(&dir);
                        dialog = dialog.set_directory(dir);
                    }
                    if let Some(dir) = dialog.pick_folder() {
                        self.export(dir);
                    }
                }
                small_check(ui, &mut self.csv.merge, &tr!("ribbon.production.merge"));
                small_check(
                    ui,
                    &mut self.csv.decimal_comma,
                    &tr!("ribbon.production.decimal_comma"),
                );
            });
            ui.add_space(10.0);
            self.production_mode(ui);
        });
    }

    /// Pohľad na stenu: výber steny ◀ W1 ▶ (ako „Wall“ v CV).
    fn group_wall_pick(&mut self, ui: &mut egui::Ui) {
        let walls: Vec<(String, f64)> = self
            .session
            .doc()
            .map(|d| {
                d.project
                    .walls
                    .iter()
                    .map(|w| (w.id.clone(), w.length))
                    .collect()
            })
            .unwrap_or_default();
        let current = self
            .wall_view
            .wall
            .clone()
            .filter(|w| walls.iter().any(|x| &x.0 == w))
            .or_else(|| walls.first().map(|w| w.0.clone()));
        let index = current
            .as_ref()
            .and_then(|c| walls.iter().position(|w| &w.0 == c));
        let mut pick: Option<String> = None;
        group(ui, &tr!("ribbon.wall.group"), |ui| {
            column(ui, |ui| {
                ui.horizontal(|ui| {
                    let prev = index.and_then(|i| i.checked_sub(1));
                    let next = index.map(|i| i + 1).filter(|&i| i < walls.len());
                    if ui
                        .add_enabled(prev.is_some(), egui::Button::new("◀").small())
                        .clicked()
                    {
                        pick = prev.and_then(|i| walls.get(i)).map(|w| w.0.clone());
                    }
                    egui::ComboBox::from_id_salt("ribbon_wall")
                        .selected_text(
                            current
                                .as_deref()
                                .map_or("–".to_owned(), |w| tr!("ribbon.wall.front", id = w)),
                        )
                        .width(130.0)
                        .show_ui(ui, |ui| {
                            for (id, length) in &walls {
                                let text = tr!(
                                    "ribbon.wall.front_length",
                                    id = id,
                                    length = crate::part_view::fmt(*length)
                                );
                                if ui
                                    .selectable_label(current.as_ref() == Some(id), text)
                                    .clicked()
                                {
                                    pick = Some(id.clone());
                                }
                            }
                        })
                        .response
                        .on_hover_text(tr!("ribbon.wall.hint"));
                    if ui
                        .add_enabled(next.is_some(), egui::Button::new("▶").small())
                        .clicked()
                    {
                        pick = next.and_then(|i| walls.get(i)).map(|w| w.0.clone());
                    }
                });
            });
        });
        if let Some(id) = pick {
            self.plan.selected_wall = Some(id.clone());
            self.wall_view.wall = Some(id);
        }
    }

    /// Zoom pôdorysu a pohľadu: výrez rámom (pôdorys), oddialiť, všetko.
    fn group_zoom(&mut self, ui: &mut egui::Ui) {
        let plan = self.tab == Tab::Plan;
        group(ui, &tr!("ribbon.zoom.group"), |ui| {
            column(ui, |ui| {
                let active = plan && self.plan.tool == Tool::ZoomWindow;
                if small_button(
                    ui,
                    Some(Icon::ZoomWindow),
                    &tr!("ribbon.zoom.window"),
                    plan,
                    active,
                )
                .on_hover_text(tr!("ribbon.zoom.window_hint"))
                .clicked()
                {
                    self.plan.set_tool(Tool::ZoomWindow);
                }
                let view = if plan {
                    &mut self.plan.view
                } else {
                    &mut self.wall_view.view
                };
                if small_button(
                    ui,
                    Some(Icon::ZoomOut),
                    &tr!("ribbon.zoom.out"),
                    true,
                    false,
                )
                .clicked()
                {
                    view.scale = (view.scale * 0.7).max(0.005);
                }
                if small_button(
                    ui,
                    Some(Icon::ZoomAll),
                    &tr!("ribbon.zoom.all"),
                    true,
                    false,
                )
                .on_hover_text(tr!("ribbon.zoom.all_hint"))
                .clicked()
                {
                    view.fit_pending = true;
                }
            });
        });
    }

    /// Režim Steny: výber, úpravy a kreslenie stien.
    fn group_wall_tools(&mut self, ui: &mut egui::Ui, has_doc: bool) {
        let tool = self.plan.tool;
        let selected = self.plan.selected_wall.clone();
        let reversed = self
            .session
            .doc()
            .is_some_and(|d| !cc_room::reversed_loops(&d.project).is_empty());
        group(ui, &tr!("ribbon.walls.select_group"), |ui| {
            column(ui, |ui| {
                if small_button(
                    ui,
                    Some(Icon::Select),
                    &tr!("ribbon.walls.select"),
                    true,
                    tool == Tool::Select,
                )
                .on_hover_text(tr!("ribbon.walls.select_hint"))
                .clicked()
                {
                    self.plan.set_tool(Tool::Select);
                }
            });
        });
        group(ui, &tr!("ribbon.walls.modify_group"), |ui| {
            column(ui, |ui| {
                if small_button(
                    ui,
                    Some(Icon::Delete),
                    &tr!("ribbon.walls.delete"),
                    has_doc && selected.is_some(),
                    false,
                )
                .on_hover_text(tr!("ribbon.walls.delete_hint"))
                .clicked()
                {
                    if let Some(id) = &selected {
                        if self.exec("wall.remove", json!({ "id": id })).is_some() {
                            self.plan.selected_wall = None;
                        }
                    }
                }
                if small_button(
                    ui,
                    Some(Icon::Turn),
                    &tr!("ribbon.walls.orient"),
                    reversed,
                    false,
                )
                .on_hover_text(tr!("ribbon.walls.orient_hint"))
                .clicked()
                {
                    self.exec("room.orient", json!({}));
                }
            });
        });
        group(ui, &tr!("ribbon.walls.draw_group"), |ui| {
            column(ui, |ui| {
                if small_button(
                    ui,
                    Some(Icon::Line),
                    &tr!("ribbon.walls.line"),
                    has_doc,
                    tool == Tool::DrawWall,
                )
                .on_hover_text(tr!("ribbon.walls.line_hint"))
                .clicked()
                {
                    self.plan.set_tool(Tool::DrawWall);
                }
                if small_button(
                    ui,
                    Some(Icon::Rectangle),
                    &tr!("ribbon.walls.rect"),
                    has_doc,
                    tool == Tool::RectRoom,
                )
                .on_hover_text(tr!("ribbon.walls.rect_hint"))
                .clicked()
                {
                    self.plan.set_tool(Tool::RectRoom);
                }
            });
        });
    }

    /// Výber výroby: všetko na CX100, alebo nesting SCM + CX100.
    fn production_mode(&mut self, ui: &mut egui::Ui) {
        let machine = self.session.doc().and_then(|d| d.machine.as_ref());
        let has_nesting = machine.is_some_and(|m| m.nesting.is_some());
        let profile_nesting =
            machine.is_some_and(|m| m.production.mode == cc_model::ProductionMode::Nesting);
        let mut nesting = self.export_nesting.unwrap_or(profile_nesting) && has_nesting;
        column(ui, |ui| {
            let a = ui
                .radio_value(&mut nesting, false, tr!("ribbon.production.all_cx100"))
                .on_hover_text(tr!("ribbon.production.all_cx100_hint"));
            let b = ui
                .add_enabled_ui(has_nesting, |ui| {
                    ui.radio_value(&mut nesting, true, tr!("ribbon.production.nesting"))
                })
                .inner
                .on_hover_text(tr!("ribbon.production.nesting_hint"))
                .on_disabled_hover_text(tr!("ribbon.production.nesting_missing"));
            if a.changed() || b.changed() {
                self.export_nesting = Some(nesting);
            }
        });
    }

    fn group_cabinets(&mut self, ui: &mut egui::Ui, has_doc: bool) {
        let selected = self
            .session
            .doc()
            .and_then(|d| d.project.cabinets.get(self.selected_index()))
            .map(|c| c.id.clone());
        let count = self.session.doc().map_or(0, |d| d.project.cabinets.len());
        group(ui, &tr!("ribbon.cabinets.group"), |ui| {
            for (icon, t, label, hint) in [
                (
                    Icon::CabinetBase,
                    "base",
                    tr!("ribbon.cabinets.base"),
                    tr!("ribbon.cabinets.base_hint"),
                ),
                (
                    Icon::CabinetWall,
                    "wall",
                    tr!("ribbon.cabinets.wall"),
                    tr!("ribbon.cabinets.wall_hint"),
                ),
                (
                    Icon::CabinetTall,
                    "tall",
                    tr!("ribbon.cabinets.tall"),
                    tr!("ribbon.cabinets.tall_hint"),
                ),
            ] {
                if big_button(ui, icon, &label, has_doc, false)
                    .on_hover_text(hint)
                    .clicked()
                {
                    self.exec("cabinet.add", json!({ "type": t }));
                }
            }
            self.catalog_button(ui, has_doc);
            column(ui, |ui| {
                let some = selected.is_some();
                if small_button(
                    ui,
                    Some(Icon::Duplicate),
                    &tr!("ribbon.cabinets.duplicate"),
                    some,
                    false,
                )
                .clicked()
                {
                    if let Some(id) = &selected {
                        self.exec("cabinet.duplicate", json!({ "id": id }));
                    }
                }
                if small_button(
                    ui,
                    Some(Icon::Delete),
                    &tr!("ribbon.cabinets.delete"),
                    some && count > 1,
                    false,
                )
                .clicked()
                {
                    if let Some(id) = &selected {
                        self.exec("cabinet.remove", json!({ "id": id }));
                    }
                }
            });
        });
    }

    /// Katalóg skriniek dielne (`catalog.toml`): ponuka po skupinách, klik pridá skrinku.
    fn catalog_button(&mut self, ui: &mut egui::Ui, has_doc: bool) {
        let items: Vec<(String, String, String, String, String)> = self
            .session
            .doc()
            .map(|d| {
                d.catalogs
                    .iter()
                    .flat_map(|c| c.catalog.items.iter().map(move |i| (c, i)))
                    .map(|(c, i)| {
                        (
                            c.name.clone(),
                            if i.group.is_empty() {
                                c.name.clone()
                            } else {
                                format!("{} – {}", c.name, i.group)
                            },
                            i.id.clone(),
                            i.name.clone(),
                            format!(
                                "{} × {} × {}",
                                crate::part_view::fmt(i.w),
                                crate::part_view::fmt(i.h),
                                crate::part_view::fmt(i.d)
                            ),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let response = big_button(
            ui,
            Icon::Catalog,
            &tr!("ribbon.cabinets.catalog"),
            has_doc,
            false,
        )
        .on_hover_text(if items.is_empty() {
            tr!("ribbon.cabinets.catalog_empty")
        } else {
            tr!("ribbon.cabinets.catalog_hint")
        });
        let popup = ui.make_persistent_id("catalog_popup");
        if response.clicked() && !items.is_empty() {
            ui.memory_mut(|m| m.toggle_popup(popup));
        }
        let selected = self.session.selection().cabinet.clone();
        let mut chosen: Option<(String, String)> = None;
        egui::popup_below_widget(
            ui,
            popup,
            &response,
            egui::PopupCloseBehavior::CloseOnClick,
            |ui| {
                ui.set_min_width(360.0);
                let mut last_group: Option<&str> = None;
                for (catalog, group, id, name, size) in &items {
                    if last_group != Some(group.as_str()) {
                        if last_group.is_some() {
                            ui.separator();
                        }
                        ui.label(egui::RichText::new(group).strong());
                        last_group = Some(group.as_str());
                    }
                    let text = format!("{name}   {size}");
                    if ui
                        .selectable_label(false, text)
                        .on_hover_text(id.as_str())
                        .clicked()
                    {
                        chosen = Some((catalog.clone(), id.clone()));
                    }
                }
            },
        );
        if let Some((from, id)) = chosen {
            let mut params = json!({ "catalog": id, "from": from });
            if let Some(after) = selected {
                params["after"] = json!(after);
            }
            self.exec("cabinet.add", params);
        }
    }

    fn group_camera(&mut self, ui: &mut egui::Ui) {
        let has_selection = {
            let s = self.session.selection();
            s.part.is_some() || s.cabinet.is_some()
        };
        group(ui, &tr!("ribbon.camera.group"), |ui| {
            for (key, label) in crate::view3d::VIEWS {
                let icon = match key {
                    "front" => Icon::Elevation,
                    "top" => Icon::Plan,
                    "iso" => Icon::Cube,
                    "selection" => Icon::Duplicate,
                    _ => Icon::Reload,
                };
                let enabled = key != "selection" || has_selection;
                // text pohľadu je kľúč prekladu alebo hotový text (tr vráti chýbajúci kľúč)
                if big_button(ui, icon, &cc_i18n::tr(label), enabled, false)
                    .on_hover_text(tr!("ribbon.camera.hint"))
                    .clicked()
                {
                    if let Err(e) = self.set_view(key) {
                        self.set_status(e, true);
                    }
                }
            }
            column(ui, |ui| {
                small_check(
                    ui,
                    &mut self.camera.perspective,
                    &tr!("ribbon.camera.perspective"),
                );
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(tr!("ribbon.camera.explode")).size(theme::BODY - 0.5),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.scene_opts.explode, 0.0..=1.0)
                            .show_value(false),
                    );
                });
            });
        });
        group(ui, &tr!("ribbon.show.group"), |ui| {
            column(ui, |ui| {
                small_check(ui, &mut self.scene_opts.backs, &tr!("ribbon.show.backs"));
                small_check(
                    ui,
                    &mut self.scene_opts.shelves,
                    &tr!("ribbon.show.shelves"),
                );
                small_check(
                    ui,
                    &mut self.scene_opts.operations,
                    &tr!("ribbon.show.operations"),
                );
            });
            column(ui, |ui| {
                small_check(ui, &mut self.scene_opts.fronts, &tr!("ribbon.show.fronts"));
                small_check(ui, &mut self.scene_opts.walls, &tr!("ribbon.show.walls"));
            });
        });
    }

    fn ribbon_tools(&mut self, ui: &mut egui::Ui) {
        let doc = self.session.doc();
        let has_doc = doc.is_some();
        let dirty = doc.is_some_and(|d| d.standard_dirty);
        group(ui, &tr!("ribbon.standard.group"), |ui| {
            if big_button(ui, Icon::Save, &tr!("ribbon.standard.save"), dirty, false).clicked()
                && self.exec("standard.save", json!({})).is_some()
            {
                self.set_status(tr!("ribbon.standard.saved"), false);
            }
            if big_button(
                ui,
                Icon::Reload,
                &tr!("ribbon.standard.reload"),
                has_doc,
                false,
            )
            .clicked()
                && self.exec("standard.reload", json!({})).is_some()
            {
                self.set_status(tr!("ribbon.standard.reloaded"), false);
            }
        });
        // Windows: .cvj sa dá otvoriť v CabinetCRAFT z ponuky pravého tlačidla
        let registered = *self
            .shell_registered
            .get_or_insert_with(crate::shell::registered);
        group(ui, &tr!("ribbon.windows.group"), |ui| {
            let label = if registered {
                tr!("ribbon.windows.unregister")
            } else {
                tr!("ribbon.windows.register")
            };
            if big_button(ui, Icon::Open, &label, cfg!(windows), registered)
                .on_hover_text(tr!("ribbon.windows.hint"))
                .clicked()
            {
                let result = if registered {
                    crate::shell::unregister()
                } else {
                    crate::shell::register()
                };
                match result {
                    Ok(()) => self.set_status(
                        if registered {
                            tr!("ribbon.windows.unregistered")
                        } else {
                            tr!("ribbon.windows.registered")
                        },
                        false,
                    ),
                    Err(e) => self.set_status(tr!("ribbon.windows.failed", error = e), true),
                }
                self.shell_registered = None;
            }
        });
        group(ui, &tr!("ribbon.control.group"), |ui| {
            column(ui, |ui| {
                let text = match self.control.port() {
                    Some(p) => tr!("ribbon.control.running", port = p),
                    None => tr!("ribbon.control.off"),
                };
                ui.label(egui::RichText::new(text).size(theme::BODY - 0.5));
                ui.label(
                    egui::RichText::new("docs/control-protocol.md")
                        .size(theme::SMALL)
                        .color(theme::TEXT_WEAK),
                );
            });
        });
        group(ui, &tr!("ribbon.language.group"), |ui| {
            column(ui, |ui| self.language_combo(ui));
        });
    }

    /// Jazyk okna: vstavané jazyky a súbory `locales/*.toml` (zoznam sa obnoví pri otvorení).
    fn language_combo(&mut self, ui: &mut egui::Ui) {
        let current = cc_i18n::language();
        let name_of = |l: &cc_i18n::Language| {
            if l.name == l.code {
                l.code.clone()
            } else {
                format!("{} ({})", l.name, l.code)
            }
        };
        let selected = self
            .languages
            .iter()
            .find(|l| l.code == current)
            .map_or_else(|| current.clone(), name_of);
        let mut pick = None;
        let combo = egui::ComboBox::from_id_salt("ribbon_language")
            .selected_text(selected)
            .width(170.0)
            .show_ui(ui, |ui| {
                for l in &self.languages {
                    let mut text = name_of(l);
                    if let Some(e) = &l.error {
                        text = format!("{text} – {e}");
                    }
                    let item = ui.add_enabled(
                        l.builtin || l.error.is_none(),
                        egui::SelectableLabel::new(l.code == current, text),
                    );
                    if item.clicked() {
                        pick = Some(l.code.clone());
                    }
                }
            });
        if combo.response.clicked() {
            self.languages = cc_i18n::languages();
        }
        combo.response.on_hover_text(tr!("ribbon.language.hint"));
        if let Some(code) = pick {
            if let Err(e) = self.set_language(&code) {
                self.set_status(e, true);
            }
        }
        if ui
            .small_button(tr!("ribbon.language.folder"))
            .on_hover_text(tr!("ribbon.language.folder_hint"))
            .clicked()
        {
            match crate::user_locales_dir() {
                Some(dir) => {
                    let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                    self.languages = cc_i18n::languages();
                }
                None => self.set_status(tr!("ribbon.language.no_folder"), true),
            }
        }
    }

    pub(crate) fn standard_combo(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = self.session.doc() else {
            return;
        };
        let current = doc.project.defaults.construction.clone();
        let names = doc.standards_available();
        let mut pick = None;
        egui::ComboBox::from_id_salt("ribbon_standard")
            .selected_text(&current)
            .width(170.0)
            .show_ui(ui, |ui| {
                for name in &names {
                    if ui.selectable_label(*name == current, name).clicked() && *name != current {
                        pick = Some(name.clone());
                    }
                }
            })
            .response
            .on_hover_text(tr!("ribbon.project.standard_hint"));
        if let Some(name) = pick {
            self.exec("standard.use", json!({ "name": name }));
        }
    }

    fn material_combo(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = self.session.doc() else {
            return;
        };
        let current = doc.project.defaults.panel_material.clone();
        let ids: Vec<String> = doc
            .materials
            .materials
            .iter()
            .map(|m| m.id.clone())
            .collect();
        let mut pick = None;
        egui::ComboBox::from_id_salt("ribbon_material")
            .selected_text(&current)
            .width(170.0)
            .show_ui(ui, |ui| {
                for id in &ids {
                    if ui.selectable_label(*id == current, id).clicked() && *id != current {
                        pick = Some(id.clone());
                    }
                }
            })
            .response
            .on_hover_text(tr!("ribbon.project.material_hint"));
        if let Some(id) = pick {
            self.exec("defaults.set", json!({ "panel_material": id }));
        }
    }

    /// Lišta pohľadov nad pracovnou plochou (Pôdorys, Pohľad, 3D, Zostavy, Výkresy, Výroba).
    pub(crate) fn view_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(8.0);
            // otvorená doska má vlastné záložky (CAM, Čelo, Hrana, Koniec, 3D, Zostavy)
            if self.tab == Tab::PartEdit {
                for (tab, icon, label, _) in crate::part_editor::PART_TABS {
                    if view_tab(ui, icon, &cc_i18n::tr(label), tab == self.part_tab).clicked() {
                        self.part_tab = tab;
                    }
                }
                return;
            }
            // editor skrinky má vlastné záložky (Rez, Čelo, Pôdorys, Bok, 3D, Zostavy)
            if self.tab == Tab::Cabinet {
                for (tab, icon, label, _) in crate::cabinet_editor::CAB_TABS {
                    if view_tab(ui, icon, &cc_i18n::tr(label), tab == self.cab_tab).clicked() {
                        self.cab_tab = tab;
                    }
                }
                return;
            }
            for (tab, icon) in VIEW_TABS {
                if view_tab(ui, icon, &tab.label(), tab.strip() == self.tab.strip()).clicked() {
                    self.tab = tab;
                }
            }
        });
    }
}

/// Jedna záložka lišty pohľadov.
fn view_tab(ui: &mut egui::Ui, icon: Icon, label: &str, active: bool) -> egui::Response {
    let font = FontId::proportional(theme::BODY - 0.5);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, theme::TEXT);
    let size = vec2(galley.size().x + 26.0, 20.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if active {
        Color32::WHITE
    } else if response.hovered() {
        theme::HOVER_BG
    } else {
        theme::BG
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0_f32, if active { theme::TEXT } else { theme::BORDER }),
        egui::StrokeKind::Inside,
    );
    icons::paint(
        ui.painter(),
        Rect::from_min_size(rect.min + vec2(4.0, 3.0), vec2(14.0, 14.0)),
        icon,
        true,
    );
    ui.painter().galley(
        pos2(rect.min.x + 21.0, rect.center().y - galley.size().y / 2.0),
        galley,
        theme::TEXT,
    );
    response
}

/// Pohľady v lište: záložka a ikona (text je [`Tab::label`]).
const VIEW_TABS: [(Tab, Icon); 6] = [
    (Tab::Plan, Icon::Plan),
    (Tab::Elevation, Icon::Elevation),
    (Tab::View3d, Icon::Cube),
    (Tab::Reports, Icon::Parts),
    (Tab::Part, Icon::Drawing),
    (Tab::Parts, Icon::Export),
];
