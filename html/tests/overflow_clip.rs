//! The page-level horizontal clip (`body { overflow-x: hidden }`) and the
//! `position: fixed` fallback policy — the pair that resolves the
//! off-viewport-parking idiom (template-compat 07: an admin-shell
//! control sidebar parked at `right: -230px`, invisible in browsers
//! because the body suppresses horizontal overflow).

use forme::model::Position;
use forme_pdf_html::{html_to_document, render_html, HtmlOptions};

#[test]
fn body_overflow_x_hidden_sets_the_page_clip() {
    let (doc, warnings) = html_to_document(
        "<html><head><style>body { overflow-x: hidden; overflow-y: auto }</style></head>\
         <body><p>content</p></body></html>",
        &HtmlOptions::default(),
    );
    assert!(
        doc.default_page.clip_content_x,
        "body overflow-x: hidden must set the page-level clip"
    );
    assert!(
        !warnings.iter().any(|w| w.contains("overflow")),
        "the honored declaration must not warn: {warnings:?}"
    );
}

#[test]
fn overflow_shorthand_and_clip_value_also_count() {
    let (doc, _) = html_to_document(
        "<html><head><style>body { overflow: hidden }</style></head>\
         <body><p>x</p></body></html>",
        &HtmlOptions::default(),
    );
    assert!(doc.default_page.clip_content_x, "overflow shorthand");
    let (doc, _) = html_to_document(
        "<html><head><style>body { overflow-x: clip }</style></head>\
         <body><p>x</p></body></html>",
        &HtmlOptions::default(),
    );
    assert!(doc.default_page.clip_content_x, "the clip value");
}

#[test]
fn overflow_x_elsewhere_warns_and_does_not_clip() {
    let (doc, warnings) = html_to_document(
        "<html><head><style>.wrapper { overflow-x: hidden }</style></head>\
         <body><div class=\"wrapper\"><p>x</p></div></body></html>",
        &HtmlOptions::default(),
    );
    assert!(!doc.default_page.clip_content_x);
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("overflow-x: hidden on <div>")),
        "{warnings:?}"
    );
}

#[test]
fn overflow_y_hidden_is_refused_by_name() {
    let (doc, warnings) = html_to_document(
        "<html><head><style>body { overflow-y: hidden }</style></head>\
         <body><p>x</p></body></html>",
        &HtmlOptions::default(),
    );
    assert!(!doc.default_page.clip_content_x);
    assert!(
        warnings.iter().any(|w| w.contains("content paginates")),
        "{warnings:?}"
    );
}

#[test]
fn position_fixed_maps_to_absolute_with_the_policy_warning() {
    let (doc, warnings) = html_to_document(
        "<html><body>\
         <div style=\"position: fixed; top: 0; right: -230px; width: 230px\"><p>parked</p></div>\
         <p>in flow</p></body></html>",
        &HtmlOptions::default(),
    );
    let body = &doc.children[0];
    let parked = &body.children[0];
    assert!(
        matches!(parked.style.position, Some(Position::Absolute)),
        "fixed must leave normal flow as absolute"
    );
    assert_eq!(parked.style.top, Some(0.0));
    assert_eq!(parked.style.right, Some(-172.5), "-230px = -172.5pt");
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("position: fixed is rendered as position: absolute")),
        "{warnings:?}"
    );
}

#[test]
fn the_admin_shell_shape_renders_with_the_clip() {
    // The 07 idiom end to end: a dark fixed block parked past the right
    // edge, hidden by the body clip. The render must succeed, carry the
    // clip, and place the parked box beyond the content box where the
    // clip erases it.
    let out = render_html(
        "<html><head><style>\
         body { overflow-x: hidden; overflow-y: auto }\
         .sidebar-bg { position: fixed; top: 0; right: -230px; width: 230px; height: 100px; background: #222d32 }\
         </style></head><body>\
         <div class=\"sidebar-bg\"></div>\
         <p>invoice content</p></body></html>",
        &HtmlOptions::default(),
    )
    .expect("render");
    assert!(out.pdf.starts_with(b"%PDF"), "valid pdf");
    // The PDF-operator assertion for the clip itself lives in the
    // engine's integration tests; here the flag on the mapped document
    // is the contract.
    let (doc, _) = html_to_document(
        "<html><head><style>body { overflow-x: hidden }</style></head><body><p>x</p></body></html>",
        &HtmlOptions::default(),
    );
    assert!(doc.default_page.clip_content_x);
}
