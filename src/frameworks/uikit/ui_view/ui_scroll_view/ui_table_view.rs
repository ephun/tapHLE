/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UITableView`, `UITableViewCell` and `UITableViewController`.
//!
//! A real table view recycles a handful of cells across thousands of rows and
//! only ever keeps the visible ones alive. This one **builds every row up
//! front** and lays them out as ordinary subviews of the scroll view.
//!
//! That is the deliberate trade. It makes the data source and delegate
//! protocols, cell reuse, selection and scrolling all work with no
//! visible-rectangle bookkeeping, at the cost of being wrong for a table with
//! thousands of rows — where it will spend memory and time proportional to the
//! whole table rather than the screen. The menus and track lists this exists to
//! serve have tens of rows. A `log!` fires past a threshold rather than letting
//! it quietly crawl.
//!
//! `-dequeueReusableCellWithIdentifier:` is honest about this: it always
//! returns nil, so the data source takes its "create a new cell" branch every
//! time. That is a documented-legal answer — a real table view returns nil
//! whenever its reuse queue is empty — and it is much safer than handing back a
//! cell that is still on screen.
//!
//! Resources:
//! - Apple's [UITableView](https://developer.apple.com/documentation/uikit/uitableview)

use crate::frameworks::core_graphics::{CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_index_path::{for_row_in_section, section_and_row};
use crate::frameworks::foundation::{NSInteger, NSUInteger};
use crate::objc::{
    id, impl_HostObject_with_superclass, msg, msg_class, msg_super, nil, objc_classes, release,
    retain, ClassExports, NSZonePtr,
};
use crate::Environment;

/// `UITableViewStyle`.
type UITableViewStyle = NSInteger;

/// `UITableViewCellStyle`. Only the default matters here: every style differs
/// in how it arranges labels, and this lays out one label.
type UITableViewCellStyle = NSInteger;

/// The height a row gets when the delegate does not say. Apple's default.
const DEFAULT_ROW_HEIGHT: f32 = 44.0;

/// Past this many rows, building every cell up front stops being a reasonable
/// approximation of a recycling table view.
const ROW_COUNT_WARNING_THRESHOLD: NSUInteger = 500;

struct UITableViewHostObject {
    superclass: crate::frameworks::uikit::ui_view::ui_scroll_view::UIScrollViewHostObject,
    /// Weak, as UIKit's are: a table view does not own its data source or
    /// delegate, and owning them here would make the usual
    /// controller-owns-table-owns-controller arrangement a cycle that never
    /// deallocates.
    data_source: id,
    delegate: id,
    row_height: f32,
    /// Remembered so a getter returns what was set; no header or footer is
    /// drawn. UIKit's defaults for both are 22 points in a grouped table and
    /// 0 in a plain one, and tapHLE lays out neither, so 0 is the honest start.
    section_header_height: f32,
    section_footer_height: f32,
    /// Whether the rows have ever been built. A table loads them the first
    /// time it is laid out, so this says whether that has happened yet.
    has_loaded: bool,
    /// The cells currently built, in display order, each retained.
    cells: Vec<id>,
    /// Parallel to `cells`: the index path each was built for, each retained.
    index_paths: Vec<id>,
}
impl_HostObject_with_superclass!(UITableViewHostObject);
impl Default for UITableViewHostObject {
    fn default() -> Self {
        UITableViewHostObject {
            superclass: Default::default(),
            data_source: nil,
            delegate: nil,
            row_height: DEFAULT_ROW_HEIGHT,
            section_header_height: 0.0,
            section_footer_height: 0.0,
            has_loaded: false,
            cells: Vec::new(),
            index_paths: Vec::new(),
        }
    }
}

#[derive(Default)]
struct UITableViewCellHostObject {
    superclass: crate::frameworks::uikit::ui_view::UIViewHostObject,
    /// `UILabel*`, retained. Created lazily, as UIKit's is.
    text_label: id,
    /// `UIView*`, retained.
    content_view: id,
    /// `NSString*`, retained.
    reuse_identifier: id,
}
impl_HostObject_with_superclass!(UITableViewCellHostObject);

/// Ask the data source and delegate for the table's contents and build a cell
/// for every row.
///
/// Everything about the table's geometry follows from this, so `-reloadData` is
/// the only thing that has to be called after the data changes — which is also
/// true of the real one.
fn reload(env: &mut Environment, table: id) {
    env.objc
        .borrow_mut::<UITableViewHostObject>(table)
        .has_loaded = true;

    // Tear down what is there. The cells are subviews, so they have to leave
    // the hierarchy as well as the list, or they would keep drawing.
    let (old_cells, old_paths) = {
        let host_object = env.objc.borrow_mut::<UITableViewHostObject>(table);
        (
            std::mem::take(&mut host_object.cells),
            std::mem::take(&mut host_object.index_paths),
        )
    };
    for cell in old_cells {
        () = msg![env; cell removeFromSuperview];
        release(env, cell);
    }
    for path in old_paths {
        release(env, path);
    }

    let data_source = env.objc.borrow::<UITableViewHostObject>(table).data_source;
    if data_source == nil {
        return;
    }

    // -numberOfSectionsInTableView: is optional; one section is the default.
    let sections_selector = env
        .objc
        .lookup_selector("numberOfSectionsInTableView:")
        .unwrap();
    let has_sections: bool = msg![env; data_source respondsToSelector:sections_selector];
    let sections: NSInteger = if has_sections {
        msg![env; data_source numberOfSectionsInTableView:table]
    } else {
        1
    };

    let bounds: CGRect = msg![env; table bounds];
    let default_height = env.objc.borrow::<UITableViewHostObject>(table).row_height;
    let delegate = env.objc.borrow::<UITableViewHostObject>(table).delegate;
    // The selector is only registered once something in the app names it. An
    // app whose delegate does not implement it never does, so there is nothing
    // to look up and equally nothing that could respond to it.
    let height_selector = env
        .objc
        .lookup_selector("tableView:heightForRowAtIndexPath:");
    let delegate_sets_height: bool = delegate != nil
        && match height_selector {
            Some(height_selector) => msg![env; delegate respondsToSelector:height_selector],
            None => false,
        };

    let mut y = 0.0f32;
    let mut total_rows: NSUInteger = 0;
    for section in 0..sections.max(0) {
        let rows: NSInteger =
            msg![env; data_source tableView:table numberOfRowsInSection:(section as NSUInteger)];
        for row in 0..rows.max(0) {
            let index_path = for_row_in_section(env, row as NSUInteger, section as NSUInteger);
            retain(env, index_path);

            let cell: id = msg![env; data_source tableView:table cellForRowAtIndexPath:index_path];
            if cell == nil {
                // A data source that returns nil is a bug in the app, but
                // aborting here would blame the table view. Skip the row and
                // keep the rest of the table.
                log!("Warning: table view data source returned nil for a row, skipping it");
                release(env, index_path);
                continue;
            }
            retain(env, cell);

            let height: f32 = if delegate_sets_height {
                msg![env; delegate tableView:table heightForRowAtIndexPath:index_path]
            } else {
                default_height
            };

            let frame = CGRect {
                origin: CGPoint { x: 0.0, y },
                size: CGSize {
                    width: bounds.size.width,
                    height,
                },
            };
            () = msg![env; cell setFrame:frame];
            () = msg![env; table addSubview:cell];

            let host_object = env.objc.borrow_mut::<UITableViewHostObject>(table);
            host_object.cells.push(cell);
            host_object.index_paths.push(index_path);

            y += height;
            total_rows += 1;
        }
    }

    log_dbg!(
        "Table view {:?} reloaded: {} sections, {} rows, content height {}",
        table,
        sections,
        total_rows,
        y
    );

    if total_rows > ROW_COUNT_WARNING_THRESHOLD {
        log!(
            "Warning: a table view built {} rows up front. tapHLE's UITableView does not recycle cells, so this costs memory and time proportional to the whole table rather than the screen.",
            total_rows
        );
    }

    let content_size = CGSize {
        width: bounds.size.width,
        height: y,
    };
    () = msg![env; table setContentSize:content_size];
}

/// Which row, if any, is under a point in the table's own coordinates.
fn index_path_at_point(env: &mut Environment, table: id, point: CGPoint) -> id {
    let cells = env
        .objc
        .borrow::<UITableViewHostObject>(table)
        .cells
        .clone();
    let paths = env
        .objc
        .borrow::<UITableViewHostObject>(table)
        .index_paths
        .clone();
    for (cell, path) in cells.into_iter().zip(paths) {
        let frame: CGRect = msg![env; cell frame];
        if point.x >= frame.origin.x
            && point.x < frame.origin.x + frame.size.width
            && point.y >= frame.origin.y
            && point.y < frame.origin.y + frame.size.height
        {
            return path;
        }
    }
    nil
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UITableViewCell: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UITableViewCellHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithStyle:(UITableViewCellStyle)_style
    reuseIdentifier:(id)reuse_identifier { // NSString*
    let this: id = msg_super![env; this init];
    let reuse_identifier: id = msg![env; reuse_identifier copy];
    env.objc.borrow_mut::<UITableViewCellHostObject>(this).reuse_identifier = reuse_identifier;
    this
}

// The pre-3.0 spelling, which the apps this targets still use.
- (id)initWithFrame:(CGRect)frame
    reuseIdentifier:(id)reuse_identifier { // NSString*
    let this: id = msg_super![env; this initWithFrame:frame];
    let reuse_identifier: id = msg![env; reuse_identifier copy];
    env.objc.borrow_mut::<UITableViewCellHostObject>(this).reuse_identifier = reuse_identifier;
    this
}

- (id)reuseIdentifier {
    env.objc.borrow::<UITableViewCellHostObject>(this).reuse_identifier
}

// Lazily created, as UIKit's are: an app that never touches -textLabel should
// not pay for one, and more importantly an app that adds its own subviews to
// -contentView must not find a stray empty label in front of them.
- (id)contentView {
    let existing = env.objc.borrow::<UITableViewCellHostObject>(this).content_view;
    if existing != nil {
        return existing;
    }
    let bounds: CGRect = msg![env; this bounds];
    let view: id = msg_class![env; UIView alloc];
    let view: id = msg![env; view initWithFrame:bounds];
    () = msg![env; this addSubview:view];
    env.objc.borrow_mut::<UITableViewCellHostObject>(this).content_view = view;
    view
}

- (id)textLabel {
    let existing = env.objc.borrow::<UITableViewCellHostObject>(this).text_label;
    if existing != nil {
        return existing;
    }
    let content_view: id = msg![env; this contentView];
    let bounds: CGRect = msg![env; content_view bounds];
    let label: id = msg_class![env; UILabel alloc];
    let label: id = msg![env; label initWithFrame:bounds];
    () = msg![env; content_view addSubview:label];
    env.objc.borrow_mut::<UITableViewCellHostObject>(this).text_label = label;
    label
}

// -text on the cell itself is the pre-3.0 spelling of -textLabel.text.
- (())setText:(id)text { // NSString*
    let label: id = msg![env; this textLabel];
    () = msg![env; label setText:text];
}

// Editing mode is not drawn — there is no delete control or reorder grip here —
// so the flag is accepted and dropped. An app puts its table into editing mode
// and UIKit forwards this to every cell, including an app's own subclass that
// overrides it and calls super, which is how a missing implementation here ends
// the app rather than merely leaving something undrawn.
- (())setEditing:(bool)_editing animated:(bool)_animated {
}
- (())setEditing:(bool)_editing {
}
- (())setShouldIndentWhileEditing:(bool)_should {
}
// The pre-iOS-3 image property, kept because old apps still set it. It is
// stored rather than drawn: this cell lays out one label and no image well.
- (())setImage:(id)_image { // UIImage*
}
- (id)image {
    nil
}
// The accessory views. Nothing draws them here — no disclosure chevron, no
// custom view well — so the setter is accepted and the getter answers nil,
// which is what a cell with no accessory has.
- (())setAccessoryView:(id)_view { // UIView*
}
- (id)accessoryView {
    nil
}
- (id)editingAccessoryView {
    nil
}
// The rest of the cell's decoration. None of it is drawn — this cell lays out
// one label — so each setter is accepted and each getter answers nil or the
// default. They are listed together rather than added one crash at a time,
// which is how the four above arrived.
- (())setBackgroundView:(id)_view { // UIView*
}
- (id)backgroundView {
    nil
}
- (())setSelectedBackgroundView:(id)_view { // UIView*
}
- (id)selectedBackgroundView {
    nil
}
- (())setMultipleSelectionBackgroundView:(id)_view { // UIView*
}
- (id)multipleSelectionBackgroundView {
    nil
}
- (())setIndentationLevel:(NSInteger)_level {
}
- (NSInteger)indentationLevel {
    0
}
- (())setIndentationWidth:(f32)_width {
}
- (NSInteger)accessoryType {
    0 // UITableViewCellAccessoryNone
}
- (NSInteger)selectionStyle {
    1 // UITableViewCellSelectionStyleBlue, the default
}
- (bool)isEditing {
    false
}
- (bool)isSelected {
    false
}
- (bool)isHighlighted {
    false
}
- (())setHighlighted:(bool)_highlighted {
}
- (())setHighlighted:(bool)_highlighted animated:(bool)_animated {
}
- (())setShowsReorderControl:(bool)_shows {
}
- (())setEditingAccessoryType:(NSInteger)_type {
}

- (())setSelectionStyle:(NSInteger)_style {
    // Selection is not drawn, so there is no style to honour.
}
- (())setAccessoryType:(NSInteger)_type {
    // No disclosure arrows or checkmarks are drawn.
}

- (())dealloc {
    let &UITableViewCellHostObject {
        text_label,
        content_view,
        reuse_identifier,
        ..
    } = env.objc.borrow(this);
    release(env, text_label);
    release(env, content_view);
    release(env, reuse_identifier);
    msg_super![env; this dealloc]
}

@end

@implementation UITableView: UIScrollView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UITableViewHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithFrame:(CGRect)frame
              style:(UITableViewStyle)_style {
    // Plain and grouped differ only in section header and background drawing,
    // neither of which is drawn here, so the style is accepted and ignored
    // rather than pretended to.
    msg![env; this initWithFrame:frame]
}

- (())setDataSource:(id)data_source {
    env.objc.borrow_mut::<UITableViewHostObject>(this).data_source = data_source;
}
- (id)dataSource {
    env.objc.borrow::<UITableViewHostObject>(this).data_source
}

- (())setDelegate:(id)delegate {
    env.objc.borrow_mut::<UITableViewHostObject>(this).delegate = delegate;
    () = msg_super![env; this setDelegate:delegate];
}

- (())setRowHeight:(f32)height {
    env.objc.borrow_mut::<UITableViewHostObject>(this).row_height = height;
}
- (f32)rowHeight {
    env.objc.borrow::<UITableViewHostObject>(this).row_height
}

// Section header and footer heights. tapHLE lays out a table as a flat list of
// rows and draws no section headers or footers, so these only have to be
// remembered accurately enough that an app reading back what it set gets the
// same number. Storing rather than ignoring matters because a table that sizes
// itself from these will otherwise compute a content height that disagrees with
// what it asked for.
- (())setSectionHeaderHeight:(f32)height {
    env.objc.borrow_mut::<UITableViewHostObject>(this).section_header_height = height;
}
- (f32)sectionHeaderHeight {
    env.objc.borrow::<UITableViewHostObject>(this).section_header_height
}
- (())setSectionFooterHeight:(f32)height {
    env.objc.borrow_mut::<UITableViewHostObject>(this).section_footer_height = height;
}
- (f32)sectionFooterHeight {
    env.objc.borrow::<UITableViewHostObject>(this).section_footer_height
}

// Separators are not drawn at all, so both the style and the colour are
// accepted and ignored. Saying that here beats being asked one selector at a
// time by each app that styles its table.
- (())setSeparatorStyle:(NSInteger)_style {
}
- (())setSeparatorColor:(id)_color { // UIColor*
}
- (())setSectionIndexMinimumDisplayRowCount:(NSInteger)_count {
    // The section index strip is not drawn either.
}
- (())setAllowsSelection:(bool)_allows {
}

- (())reloadData {
    reload(env, this);
}

// A table view loads its rows the first time it is laid out, not when its data
// source is set. That is why an app can build a table in a nib, point it at a
// data source, and never call -reloadData: by the time the table is on screen
// UIKit has already asked. Loading only on the first pass keeps a later layout
// from rebuilding cells underneath an app that is holding one.
- (())layoutSubviews {
    () = msg_super![env; this layoutSubviews];
    let &UITableViewHostObject { has_loaded, data_source, .. } =
        env.objc.borrow::<UITableViewHostObject>(this);
    if !has_loaded && data_source != nil {
        reload(env, this);
    }
}

// Always nil, and deliberately. A real table view returns nil whenever its
// reuse queue is empty, so every data source already handles this; handing back
// a cell that is still on screen would be far worse.
- (id)dequeueReusableCellWithIdentifier:(id)_identifier { // NSString*
    nil
}

- (id)cellForRowAtIndexPath:(id)index_path { // NSIndexPath*
    let (section, row) = section_and_row(env, index_path);
    let paths = env.objc.borrow::<UITableViewHostObject>(this).index_paths.clone();
    let cells = env.objc.borrow::<UITableViewHostObject>(this).cells.clone();
    for (path, cell) in paths.into_iter().zip(cells) {
        if section_and_row(env, path) == (section, row) {
            return cell;
        }
    }
    nil
}

- (id)indexPathForCell:(id)cell { // UITableViewCell*
    let cells = env.objc.borrow::<UITableViewHostObject>(this).cells.clone();
    let paths = env.objc.borrow::<UITableViewHostObject>(this).index_paths.clone();
    for (candidate, path) in cells.into_iter().zip(paths) {
        if candidate == cell {
            return path;
        }
    }
    nil
}

- (id)indexPathForSelectedRow {
    nil
}

- (())selectRowAtIndexPath:(id)_index_path
                  animated:(bool)_animated
            scrollPosition:(NSInteger)_position {
    // Selection is not drawn, so there is nothing to show.
}
- (())deselectRowAtIndexPath:(id)_index_path animated:(bool)_animated {
}

- (())scrollToRowAtIndexPath:(id)index_path
            atScrollPosition:(NSInteger)_position
                    animated:(bool)_animated {
    // Bring the row to the top of the visible area. The finer positions
    // (middle, bottom) are not distinguished.
    let cell: id = msg![env; this cellForRowAtIndexPath:index_path];
    if cell == nil {
        return;
    }
    let frame: CGRect = msg![env; cell frame];
    let offset = CGPoint {
        x: 0.0,
        y: frame.origin.y,
    };
    () = msg![env; this setContentOffset:offset];
}

// Touch handling. UIScrollView's own handling deals with dragging; a tap that
// lands on a row is turned into the delegate callback here.
- (())touchesEnded:(id)touches withEvent:(id)event {
    () = msg_super![env; this touchesEnded:touches withEvent:event];

    let delegate = env.objc.borrow::<UITableViewHostObject>(this).delegate;
    if delegate == nil {
        return;
    }
    let selector = env
        .objc
        .lookup_selector("tableView:didSelectRowAtIndexPath:")
        .unwrap();
    let responds: bool = msg![env; delegate respondsToSelector:selector];
    if !responds {
        return;
    }

    let touch: id = msg![env; touches anyObject];
    if touch == nil {
        return;
    }
    let point: CGPoint = msg![env; touch locationInView:this];
    let index_path = index_path_at_point(env, this, point);
    if index_path == nil {
        return;
    }
    () = msg![env; delegate tableView:this didSelectRowAtIndexPath:index_path];
}

- (())dealloc {
    let (cells, paths) = {
        let host_object = env.objc.borrow_mut::<UITableViewHostObject>(this);
        (
            std::mem::take(&mut host_object.cells),
            std::mem::take(&mut host_object.index_paths),
        )
    };
    for cell in cells {
        release(env, cell);
    }
    for path in paths {
        release(env, path);
    }
    msg_super![env; this dealloc]
}

@end

@implementation UITableViewController: UIViewController

// The controller's view *is* its table view, which is the whole point of the
// class: -view and -tableView are the same object, and setting either sets
// both. An app that adds its own subviews to -view therefore adds them to the
// table, exactly as on a device.
- (())loadView {
    let screen: id = msg_class![env; UIScreen mainScreen];
    let frame: CGRect = msg![env; screen applicationFrame];
    let table: id = msg_class![env; UITableView alloc];
    let table: id = msg![env; table initWithFrame:frame];
    () = msg![env; table setDataSource:this];
    () = msg![env; table setDelegate:this];
    () = msg![env; this setView:table];
    release(env, table);
}

// The style is accepted and dropped, for the same reason UITableView drops it:
// plain and grouped differ only in section-header and background drawing, and
// neither is drawn. Storing it would imply it changes something.
- (id)initWithStyle:(UITableViewStyle)_style {
    msg![env; this init]
}

- (id)tableView {
    msg![env; this view]
}

- (())setTableView:(id)table_view {
    () = msg![env; this setView:table_view];
}

- (())viewWillAppear:(bool)animated {
    () = msg_super![env; this viewWillAppear:animated];
    // UIKit reloads here so a controller that only sets up its data in
    // -viewDidLoad still shows it.
    let table: id = msg![env; this view];
    () = msg![env; table reloadData];
}

@end

};
