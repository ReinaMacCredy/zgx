//! A focused draw places its caret and scroll from its lines' shaped heights, not their estimates.
use zgx::{
    compose::prelude::*,
    scene::{NodeId, NodeKind},
    widgets::Ui,
};

fn line_node(ui: &Ui, root: NodeId, line: &str) -> NodeId {
    let scene = ui.scene.borrow();
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        let shown = match scene.kind(id) {
            NodeKind::Text { text, .. } => text.as_ref() == line,
            NodeKind::RichText { text } => text.text() == line,
            _ => false,
        };
        if shown {
            return id;
        }
        pending.extend(scene.children(id).iter().copied());
    }
    panic!("no line {line:?} shows");
}

#[test]
fn wrapping_turned_on_focused_places_caret_and_lines_from_shaped_heights() {
    let mut ui = Ui::new(400., 300.);
    // Two bytes a character: estimated at three rows, shaped at two.
    let first = "é".repeat(60);
    let text = format!("{first}\na\nb\nc");
    let wrap = ui.signal(false);
    let read = wrap.clone();
    let view = ui.mount(
        text_area("Draft", ui.signal(text.clone()))
            .size(300., 100.)
            .p(0.)
            .text_size(10.)
            .line_height(20.)
            .reactive_style(move || Styles::new().text_wrap(read.get()))
            .span_highlighter(|_, state| (Vec::new(), state)),
    );
    ui.prepare_frame();
    ui.input.focus(&ui.scene, Some(view.node()));
    let editor = ui.focused_editor().unwrap();
    editor.set_selection(text.len(), text.len());
    ui.prepare_frame();
    wrap.set(true);
    ui.prepare_frame();
    let root = ui.scene.borrow().bounds(view.node());
    let caret = ui.scene.borrow().bounds(editor.caret);
    let last = ui.scene.borrow().bounds(line_node(&ui, view.node(), "c"));
    let top = ui
        .scene
        .borrow()
        .bounds(line_node(&ui, view.node(), &first));
    assert_eq!(top.y, root.y, "the text fits: nothing scrolled");
    assert_eq!(last.y, root.y + 80.);
    assert_eq!(caret.y, last.y, "the caret on its line");
}
