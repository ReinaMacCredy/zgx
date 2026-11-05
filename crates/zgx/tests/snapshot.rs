//! A node's picture as the window last showed it: the host takes it
//! (`Ui::on_snapshot`), given the node's box; without one there is none.
use std::{cell::RefCell, rc::Rc, sync::Arc};
use zgx::{compose::prelude::*, image::ImageData, scene::Rect, widgets::Ui};

#[test]
fn a_snapshot_is_the_hosts_picture_of_the_nodes_box() {
    let mut ui = Ui::new(400., 300.);
    let view = ui.mount(
        column()
            .p(10.)
            .child(div().id("card").w(120.).h(60.).bg(rgb(0x334455)))
            .child(div().id("away").w(120.).h(60.).hidden()),
    );
    ui.prepare_frame();
    let card = view.find("card").unwrap();
    // No host taking pictures: none.
    assert!(ui.snapshot(card, 100).is_none());
    let asked: Rc<RefCell<Vec<(Rect, u32)>>> = Rc::default();
    let log = asked.clone();
    ui.on_snapshot(move |rect, max_width| {
        log.borrow_mut().push((rect, max_width));
        Some(Arc::new(ImageData::new(2, 1, vec![255u8; 8]).unwrap()))
    });
    let picture = ui.snapshot(card, 100).expect("a picture");
    assert_eq!((picture.width(), picture.height()), (2, 1));
    assert_eq!(
        *asked.borrow(),
        vec![(Rect::new(10., 10., 120., 60.), 100)],
        "the node's box, in the window"
    );
    // Hidden (a tab not shown): its box is not what shows, so none.
    let away = view.find("away").unwrap();
    assert!(ui.snapshot(away, 100).is_none());
    assert_eq!(asked.borrow().len(), 1, "the host is not asked");
    // Gone from the scene: none.
    view.unmount();
    ui.prepare_frame();
    assert!(ui.snapshot(card, 100).is_none());
}

/// A node in a popup is pictured from the popup's own surface where the
/// host shows one (past the window's edge, it is the only place it shows),
/// else from the window's.
#[test]
fn a_popups_node_is_pictured_from_its_own_surface() {
    let mut ui = Ui::new(200., 100.);
    // A host showing popups in windows of their own.
    ui.set_popup_screens(vec![Rect::new(0., 0., 1000., 800.)]);
    let open = ui.signal(true);
    let view = ui.mount(
        popover("Far", open, div().id("anchor").w(40.).h(20.))
            .child(div().id("inside").w(300.).h(60.).bg(rgb(0x334455))),
    );
    ui.prepare_frame();
    let inside = view.find("inside").unwrap();
    let window: Rc<RefCell<usize>> = Rc::default();
    let seen = window.clone();
    ui.on_snapshot(move |_, _| {
        *seen.borrow_mut() += 1;
        Some(Arc::new(ImageData::new(1, 1, vec![1u8; 4]).unwrap()))
    });
    // No popup surfaces: the window's picture.
    assert_eq!(ui.snapshot(inside, 50).unwrap().width(), 1);
    let asked: Rc<RefCell<Vec<Rect>>> = Rc::default();
    let log = asked.clone();
    ui.on_popup_snapshot(move |_, rect, _| {
        log.borrow_mut().push(rect);
        Some(Arc::new(ImageData::new(3, 1, vec![2u8; 12]).unwrap()))
    });
    assert_eq!(
        ui.snapshot(inside, 50).unwrap().width(),
        3,
        "the popup's own"
    );
    assert_eq!(asked.borrow().len(), 1);
    assert_eq!(*window.borrow(), 1, "the window isn't asked");
    // A node in no popup: the window's, the popups' host not asked.
    let anchor = view.find("anchor").unwrap();
    assert_eq!(ui.snapshot(anchor, 50).unwrap().width(), 1);
    assert_eq!(asked.borrow().len(), 1);
}

/// What the window last showed has a modal over the node: until a frame shows
/// it gone. What's outside the modal's box, and the modal's own, aren't covered;
/// a popup in a surface of its own is no part of the window's picture.
#[test]
fn a_node_is_covered_while_the_window_last_showed_an_overlay_over_it() {
    let mut ui = Ui::new(400., 300.);
    let open = ui.signal(false);
    let view = ui.mount(
        column()
            .child(div().id("page").w(400.).h(300.).bg(rgb(0x334455)))
            .child(modal("Over it", open.clone()).child(div().id("in-modal").w(100.).h(50.))),
    );
    ui.prepare_frame();
    let page = view.find("page").unwrap();
    assert!(!ui.covered(page), "nothing over it");
    open.set(true);
    ui.prepare_frame();
    assert!(ui.covered(page), "a modal over it");
    let inside = view.find("in-modal").unwrap();
    assert!(!ui.covered(inside), "not by the modal it is in");
    open.set(false);
    assert!(ui.covered(page), "what the window last showed still has it");
    ui.prepare_frame();
    assert!(!ui.covered(page), "shut, in the frame since");

    let mut ui = Ui::new(400., 300.);
    ui.set_popup_screens(vec![Rect::new(0., 0., 1000., 800.)]);
    let open = ui.signal(true);
    let view = ui.mount(
        column()
            .child(popover("Near", open.clone(), div().id("anchor").w(40.).h(20.)).child(div().w(300.).h(200.)))
            .child(div().id("page").w(400.).h(280.)),
    );
    ui.prepare_frame();
    let page = view.find("page").unwrap();
    assert!(ui.covered(page), "with no surfaces of its own, the window draws it");
    ui.on_popup_snapshot(|_, _, _| None);
    ui.prepare_frame();
    assert!(!ui.covered(page), "in a surface of its own");

    // A layer is the window's size whatever it holds: only what it holds covers.
    let shell = |held: bool| {
        let mut ui = Ui::new(400., 300.);
        let open = ui.signal(true);
        let layer = modal("Notices", open).layer();
        let layer = if held { layer.child(div().w(50.).h(50.).bg(rgb(0x334455))) } else { layer };
        let view = ui.mount(
            column()
                .child(div().id("page").w(400.).h(300.))
                .child(layer),
        );
        ui.prepare_frame();
        let page = view.find("page").unwrap();
        ui.covered(page)
    };
    assert!(!shell(false), "an empty layer");
    assert!(shell(true), "what a layer holds");
}
