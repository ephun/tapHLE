/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The Log / Output panel along the bottom of the window.
//!
//! This is the developer's half of the frontend, and it is hidden by default
//! so that the other half stays uncluttered. It is a viewer, not a terminal:
//! there is nothing to type into it, because the emulator has no console to
//! type at.
//!
//! Two properties matter more than anything else here.
//!
//! *It must never be the reason the interface stutters.* A talkative app
//! produces thousands of lines a second. Only the rows actually on screen are
//! drawn, and the set of lines passing the current filter is maintained
//! incrementally — each new line is tested once, when it arrives, rather than
//! the whole buffer being rescanned every frame.
//!
//! *It must keep receiving while collapsed.* Nothing in this file is
//! connected to whether the panel is shown: the lines land in
//! [crate::logstore] as they arrive, from the reader threads, and this only
//! ever reads them. Collapsing the panel stops it being drawn and does
//! nothing else.

use egui::{Sense, Ui, Vec2};

use crate::logstore::{LogLevel, LogStore};
use crate::theme;
use crate::ui::{self, Action};

/// What the panel is showing, and what is selected in it.
pub struct LogView {
    pub search: String,
    pub show_debug: bool,
    pub show_info: bool,
    pub show_warning: bool,
    pub show_error: bool,
    /// Show only lines from the selected app's runs.
    pub only_selected_app: bool,
    /// Keep the newest line in view.
    pub follow_tail: bool,
    /// Stop adding to the view. Lines still arrive and are still stored.
    pub paused: bool,
    pub show_timestamps: bool,
    /// The module filter, empty for all.
    pub module_filter: String,

    /// Sequence numbers passing the filter, in order.
    matched: Vec<u64>,
    /// The first sequence number not yet tested against the filter.
    scanned_to: u64,
    /// The filter these results belong to.
    filter_key: String,
    /// Selected rows, as an inclusive range of sequence numbers.
    selection: Option<(u64, u64)>,
    /// Where a shift-click extends from.
    anchor: Option<u64>,
}

impl Default for LogView {
    fn default() -> Self {
        LogView {
            search: String::new(),
            show_debug: true,
            show_info: true,
            show_warning: true,
            show_error: true,
            only_selected_app: false,
            follow_tail: true,
            paused: false,
            show_timestamps: true,
            module_filter: String::new(),
            matched: Vec::new(),
            scanned_to: 0,
            filter_key: String::new(),
            selection: None,
            anchor: None,
        }
    }
}

impl LogView {
    fn level_allowed(&self, level: LogLevel) -> bool {
        match level {
            LogLevel::Debug => self.show_debug,
            LogLevel::Info => self.show_info,
            LogLevel::Warning => self.show_warning,
            LogLevel::Error => self.show_error,
        }
    }

    /// A string that changes whenever the filter does, so the cached results
    /// can be thrown away exactly when they stop being valid.
    fn current_filter_key(&self, run_ids: &[u64]) -> String {
        format!(
            "{}|{}|{}{}{}{}|{}|{:?}",
            self.search.to_lowercase(),
            self.module_filter.to_lowercase(),
            self.show_debug as u8,
            self.show_info as u8,
            self.show_warning as u8,
            self.show_error as u8,
            self.only_selected_app as u8,
            run_ids,
        )
    }

    fn matches(&self, line: &crate::logstore::LogLine, run_ids: &[u64]) -> bool {
        if !self.level_allowed(line.level) {
            return false;
        }
        if self.only_selected_app {
            match line.origin.run_id() {
                Some(id) if run_ids.contains(&id) => (),
                _ => return false,
            }
        }
        if !self.module_filter.trim().is_empty() {
            let wanted = self.module_filter.trim().to_lowercase();
            let module = line.module.as_deref().unwrap_or("").to_lowercase();
            if !module.contains(&wanted) {
                return false;
            }
        }
        let needle = self.search.trim().to_lowercase();
        if !needle.is_empty() {
            let haystack = line.message.to_lowercase();
            let module = line.module.as_deref().unwrap_or("").to_lowercase();
            if !haystack.contains(&needle) && !module.contains(&needle) {
                return false;
            }
        }
        true
    }

    /// Bring the filtered set up to date with the store.
    ///
    /// Only the lines that have arrived since the last call are examined,
    /// unless the filter itself changed, in which case everything held is
    /// re-examined once.
    fn refresh(&mut self, store: &LogStore, run_ids: &[u64]) {
        let key = self.current_filter_key(run_ids);
        if key != self.filter_key {
            self.filter_key = key;
            self.matched.clear();
            self.scanned_to = store.first_seq();
        }
        // Lines the store has dropped can no longer be shown.
        let first = store.first_seq();
        if self.scanned_to < first {
            self.scanned_to = first;
        }
        if let Some(cut) = self.matched.iter().position(|seq| *seq >= first) {
            if cut > 0 {
                self.matched.drain(..cut);
            }
        } else if !self.matched.is_empty() && self.matched[self.matched.len() - 1] < first {
            self.matched.clear();
        }

        if self.paused {
            return;
        }
        for seq in self.scanned_to..store.next_seq() {
            if let Some(line) = store.get(seq) {
                if self.matches(line, run_ids) {
                    self.matched.push(seq);
                }
            }
        }
        self.scanned_to = store.next_seq();
    }

    pub fn select_all(&mut self) {
        self.selection = match (self.matched.first(), self.matched.last()) {
            (Some(first), Some(last)) => Some((*first, *last)),
            _ => None,
        };
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
        self.anchor = None;
    }

    fn is_selected(&self, seq: u64) -> bool {
        matches!(self.selection, Some((from, to)) if seq >= from && seq <= to)
    }

    /// The visible lines, or just the selected ones, as text.
    pub fn text(&self, store: &LogStore, selection_only: bool) -> String {
        let mut text = String::new();
        for seq in &self.matched {
            if selection_only && !self.is_selected(*seq) {
                continue;
            }
            let Some(line) = store.get(*seq) else {
                continue;
            };
            if self.show_timestamps {
                text.push_str(&crate::timefmt::format_clock(line.millis));
                text.push(' ');
            }
            text.push_str(line.level.marker());
            text.push(' ');
            text.push_str(&line.full_text());
            text.push('\n');
        }
        text
    }

    pub fn has_selection(&self) -> bool {
        self.selection.is_some()
    }

    pub fn visible_count(&self) -> usize {
        self.matched.len()
    }
}

/// Draw the panel.
///
/// `run_ids` are the runs belonging to the selected app, used by the "this
/// app only" filter.
pub fn show(
    ui: &mut Ui,
    view: &mut LogView,
    store: &LogStore,
    run_ids: &[u64],
    actions: &mut Vec<Action>,
) {
    view.refresh(store, run_ids);
    // The whole panel is painted first. Doing it inside `rows` painted over
    // the toolbar, because a Ui's max_rect stays the full region however far
    // down the cursor has moved.
    ui.painter()
        .rect_filled(ui.max_rect(), 0.0, theme::LIGHT.log_background);
    toolbar(ui, view, store, actions);
    theme::hairline(ui);
    rows(ui, view, store);
}

fn toolbar(ui: &mut Ui, view: &mut LogView, store: &LogStore, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Log").strong());

        ui.add_space(6.0);
        let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(13.0), Sense::hover());
        ui::draw_icon(
            ui.painter(),
            icon_rect,
            ui::Icon::Search,
            theme::LIGHT.text_dim,
        );
        ui.add(
            egui::TextEdit::singleline(&mut view.search)
                .desired_width(150.0)
                .hint_text("Search"),
        );

        ui.separator();
        for (flag, level) in [
            (&mut view.show_error, LogLevel::Error),
            (&mut view.show_warning, LogLevel::Warning),
            (&mut view.show_info, LogLevel::Info),
            (&mut view.show_debug, LogLevel::Debug),
        ] {
            ui.checkbox(flag, level.label())
                .on_hover_text(format!("Show {} lines", level.label().to_lowercase()));
        }

        ui.separator();
        ui.checkbox(&mut view.only_selected_app, "This app only")
            .on_hover_text("Show only output from runs of the selected app");
        ui.add(
            egui::TextEdit::singleline(&mut view.module_filter)
                .desired_width(130.0)
                .hint_text("Subsystem"),
        )
        .on_hover_text("Filter by the tapHLE module a line came from");

        // The controls on the right, laid out from that edge inwards.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(4.0);
            if ui.button("Hide").clicked() {
                actions.push(Action::ShowLogPanel(false));
            }
            if ui
                .button("Save…")
                .on_hover_text("Write the lines shown here to a file")
                .clicked()
            {
                actions.push(Action::SaveLog);
            }
            if ui
                .button("Clear")
                .on_hover_text("Discard the lines held in memory")
                .clicked()
            {
                actions.push(Action::ClearLog);
            }
            if ui
                .add_enabled(view.has_selection(), egui::Button::new("Copy"))
                .on_hover_text("Copy the selected lines")
                .clicked()
            {
                actions.push(Action::CopyLogSelection);
            }
            ui.separator();
            ui.checkbox(&mut view.paused, "Pause")
                .on_hover_text("Stop updating the view. Output is still recorded.");
            ui.checkbox(&mut view.follow_tail, "Follow");
            ui.checkbox(&mut view.show_timestamps, "Times")
                .on_hover_text(if crate::timefmt::times_are_utc() {
                    "Show the time each line arrived (UTC on this platform)"
                } else {
                    "Show the time each line arrived"
                });
            ui.separator();
            let dropped = store.dropped();
            let summary = if dropped > 0 {
                format!(
                    "{} shown, {} kept, {dropped} dropped",
                    view.visible_count(),
                    store.len()
                )
            } else {
                format!("{} shown of {}", view.visible_count(), store.len())
            };
            ui.label(
                egui::RichText::new(summary)
                    .small()
                    .color(theme::LIGHT.text_dim),
            );
        });
    });
}

fn rows(ui: &mut Ui, view: &mut LogView, store: &LogStore) {
    let palette = &theme::LIGHT;
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace) + 2.0;

    // Ctrl+A and Ctrl+C work when the pointer is over the panel, which is
    // what a person expects of a pane they are reading.
    let hovered = ui.rect_contains_pointer(ui.max_rect());
    if hovered {
        let (select_all, copy) = ui.input(|input| {
            (
                input.modifiers.command && input.key_pressed(egui::Key::A),
                input.modifiers.command && input.key_pressed(egui::Key::C),
            )
        });
        if select_all {
            view.select_all();
        }
        if copy {
            let text = view.text(store, true);
            if !text.is_empty() {
                ui.ctx().copy_text(text);
            }
        }
    }

    if store.is_empty() {
        ui.add_space(10.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("No output yet. Start an app to see its log here.")
                    .color(palette.text_dim),
            );
        });
        return;
    }

    // Following the tail is egui's own `stick_to_bottom`. Setting a huge
    // scroll offset instead looks equivalent and is not: `show_rows` turns
    // the offset into a row index, and an offset of f32::MAX turns into a
    // row index past the end, so nothing at all gets drawn.
    let scroll = egui::ScrollArea::both()
        .auto_shrink([false, false])
        .stick_to_bottom(view.follow_tail);
    scroll.show_rows(ui, row_height, view.matched.len(), |ui, row_range| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut clicked: Option<(u64, bool)> = None;
        for row in row_range {
            let Some(&seq) = view.matched.get(row) else {
                continue;
            };
            let Some(line) = store.get(seq) else { continue };

            let width = ui.available_width().max(900.0);
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(width, row_height), Sense::click());
            if view.is_selected(seq) {
                ui.painter().rect_filled(rect, 0.0, palette.selection_fill);
            } else if response.hovered() {
                ui.painter().rect_filled(rect, 0.0, palette.hover_fill);
            }
            if response.clicked() {
                let extend = ui.input(|input| input.modifiers.shift);
                clicked = Some((seq, extend));
            }

            let colour = match line.level {
                LogLevel::Error => palette.error,
                LogLevel::Warning => palette.warning,
                LogLevel::Debug => palette.text_dim,
                LogLevel::Info => palette.text,
            };
            let mut x = rect.left() + 6.0;
            let mut put = |text: &str, colour: egui::Color32, gap: f32| {
                let galley = ui
                    .painter()
                    .layout_no_wrap(text.to_string(), font.clone(), colour);
                let advance = galley.size().x;
                ui.painter()
                    .galley(egui::pos2(x, rect.top() + 1.0), galley, colour);
                x += advance + gap;
            };
            if view.show_timestamps {
                put(
                    &crate::timefmt::format_clock(line.millis),
                    palette.text_dim,
                    8.0,
                );
            }
            if line.level >= LogLevel::Warning {
                put(line.level.marker(), colour, 6.0);
            }
            // With more than one app's output in view, say which is which.
            if !view.only_selected_app {
                if let crate::logstore::LogOrigin::Run { .. } = &line.origin {
                    put(line.origin.label(), palette.accent, 6.0);
                }
            }
            if let Some(module) = &line.module {
                // The `tapHLE::` prefix is on every line and carries no
                // information; the part after it is the subsystem.
                let short = module.strip_prefix("tapHLE::").unwrap_or(module);
                put(short, palette.text_dim, 6.0);
            }
            put(&line.message, colour, 0.0);
        }
        if let Some((seq, extend)) = clicked {
            if extend {
                let anchor = view.anchor.unwrap_or(seq);
                view.selection = Some((anchor.min(seq), anchor.max(seq)));
            } else {
                view.anchor = Some(seq);
                view.selection = Some((seq, seq));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logstore::{LogOrigin, LogStore};

    fn store_with(lines: &[&str]) -> LogStore {
        let mut store = LogStore::default();
        for line in lines {
            store.push_raw(LogOrigin::Frontend, line);
        }
        store
    }

    #[test]
    fn severity_filters_hide_and_show_lines() {
        let store = store_with(&[
            "ordinary line",
            "tapHLE::x: Warning: something",
            "thread 'main' panicked at lib.rs",
        ]);
        let mut view = LogView::default();
        view.refresh(&store, &[]);
        assert_eq!(view.visible_count(), 3);

        view.show_info = false;
        view.show_warning = false;
        view.refresh(&store, &[]);
        assert_eq!(view.visible_count(), 1, "only the error should remain");
    }

    #[test]
    fn search_narrows_to_matching_lines() {
        let store = store_with(&["alpha", "beta", "alphabet"]);
        let mut view = LogView {
            search: "alpha".to_string(),
            ..LogView::default()
        };
        view.refresh(&store, &[]);
        assert_eq!(view.visible_count(), 2);
    }

    /// The module prefix is the only structure the emulator's output has, so
    /// filtering on it is how a subsystem is isolated.
    #[test]
    fn the_subsystem_filter_uses_the_module_prefix() {
        let store = store_with(&[
            "tapHLE::frameworks::uikit: a",
            "tapHLE::mem: b",
            "no module here",
        ]);
        let mut view = LogView {
            module_filter: "uikit".to_string(),
            ..LogView::default()
        };
        view.refresh(&store, &[]);
        assert_eq!(view.visible_count(), 1);
    }

    /// Pausing must freeze what is shown without stopping what is recorded;
    /// that distinction is the whole point of the control.
    #[test]
    fn pausing_freezes_the_view_but_not_the_store() {
        let mut store = store_with(&["one"]);
        let mut view = LogView::default();
        view.refresh(&store, &[]);
        assert_eq!(view.visible_count(), 1);

        view.paused = true;
        store.push_raw(LogOrigin::Frontend, "two");
        store.push_raw(LogOrigin::Frontend, "three");
        view.refresh(&store, &[]);
        assert_eq!(view.visible_count(), 1, "the view should not have grown");
        assert_eq!(store.len(), 3, "the store should still have every line");

        view.paused = false;
        view.refresh(&store, &[]);
        assert_eq!(view.visible_count(), 3, "resuming should catch up");
    }

    /// Filtering is incremental, so the results must survive the buffer
    /// dropping its oldest lines. Getting this wrong shows the wrong text
    /// against the wrong row once a long run fills the buffer.
    #[test]
    fn the_filter_survives_lines_being_dropped() {
        let mut store = LogStore::default();
        store.set_capacity(1000);
        let mut view = LogView::default();
        for i in 0..2500 {
            store.push_raw(LogOrigin::Frontend, &format!("line {i}"));
            if i % 100 == 0 {
                view.refresh(&store, &[]);
            }
        }
        view.refresh(&store, &[]);
        assert_eq!(view.visible_count(), store.len());
        for seq in &view.matched {
            assert!(store.get(*seq).is_some(), "seq {seq} should still exist");
        }
    }

    /// Changing the filter has to re-examine what is held, not just what
    /// arrives next.
    #[test]
    fn changing_the_filter_rescans_what_is_held() {
        let store = store_with(&["alpha", "beta"]);
        let mut view = LogView::default();
        view.refresh(&store, &[]);
        assert_eq!(view.visible_count(), 2);
        view.search = "beta".to_string();
        view.refresh(&store, &[]);
        assert_eq!(view.visible_count(), 1);
        view.search.clear();
        view.refresh(&store, &[]);
        assert_eq!(view.visible_count(), 2);
    }

    #[test]
    fn selected_lines_can_be_taken_as_text() {
        let store = store_with(&["alpha", "beta", "gamma"]);
        let mut view = LogView {
            show_timestamps: false,
            ..LogView::default()
        };
        view.refresh(&store, &[]);
        view.selection = Some((1, 2));
        let text = view.text(&store, true);
        assert!(text.contains("beta"));
        assert!(text.contains("gamma"));
        assert!(!text.contains("alpha"));
    }

    /// Only lines belonging to the chosen runs pass the per-app filter;
    /// frontend messages are not part of an app's output.
    #[test]
    fn the_per_app_filter_keeps_only_that_app() {
        let mut store = LogStore::default();
        store.push_raw(LogOrigin::Frontend, "frontend line");
        store.push_raw(
            LogOrigin::Run {
                id: 7,
                app: "Ricky".into(),
            },
            "app line",
        );
        let mut view = LogView {
            only_selected_app: true,
            ..LogView::default()
        };
        view.refresh(&store, &[7]);
        assert_eq!(view.visible_count(), 1);
    }
}
