//! Operations have no generator preconditions: an unavailable activation is a
//! no-op, so shrinking can remove any earlier operation.
use super::*;
use crate::strategies::{self, PagerOp};
use proptest::prelude::*;

fn key(state: &mut PagerState, code: KeyCode) {
    state.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE));
}
fn selected(state: &PagerState) -> Option<usize> {
    state
        .active
        .map(|i| state.interactive_blocks[i].block_index)
}
/// The diagram block holding flat line `line`, if `i` can switch it and it
/// is in image form.
fn shown_image_at(state: &PagerState, line: usize) -> Option<usize> {
    let bi = state
        .interactive_blocks
        .iter()
        .find(|e| (e.flat_line_index..e.flat_line_end).contains(&line))?
        .block_index;
    (state.can_switch_diagram(bi) && state.diagram_shows_image(bi)).then_some(bi)
}
fn check(state: &PagerState) {
    assert!(state.scroll <= state.max_scroll());
    if let Some(i) = state.active {
        assert!(i < state.interactive_blocks.len());
        assert!(matches!(
            state.content[state.interactive_blocks[i].block_index],
            RenderedBlock::Diagram { .. } | RenderedBlock::Image { .. }
        ));
    }
    // Every image row belongs to its image's interactive block.
    for (i, line) in state.flat_lines.iter().enumerate() {
        if let FlatLine::ImageRow { block_index, .. } = line {
            assert!(
                state
                    .interactive_blocks
                    .iter()
                    .any(|e| e.block_index == *block_index
                        && (e.flat_line_index..e.flat_line_end).contains(&i))
            );
        }
    }
    let matches: Vec<_> = if state.search_query.is_empty() {
        vec![]
    } else {
        state
            .flat_lines
            .iter()
            .enumerate()
            .filter_map(|(i, line)| {
                PagerState::flat_line_text(line)
                    .to_lowercase()
                    .contains(&state.search_query.to_lowercase())
                    .then_some(i)
            })
            .collect()
    };
    assert_eq!(state.search_matches, matches);
    if let Some(i) = state.search_current {
        assert!(i < matches.len());
    } else {
        assert!(matches.is_empty());
    }
}

proptest! {
    #![proptest_config(strategies::model_config(64))]
    #[test]
    fn property_pager_model(content in strategies::rendered_blocks(), ops in strategies::pager_ops(), width in strategies::width(), images in any::<bool>()) {
        let mut state = PagerState::new(content,20,width,100,crate::theme::Theme::default_theme());
        if images {
            // Graphics on: `test.png` takes several placeholder rows.
            let mut store = crate::images::ImageStore::for_tests("/docs".into(),(10,20));
            store.insert_ready("test.png",(400,200));
            state = state.with_images(store);
        }
        // Activating image placeholders must never spawn a desktop application.
        state.opener = None;
        check(&state);
        for op in ops {
            match op {
                PagerOp::Key(which) => {
                    let old_scroll = state.scroll;
                    let max = state.flat_lines.len().saturating_sub(usize::from(state.terminal_height));
                    let page = usize::from(state.terminal_height).saturating_sub(1).max(1);
                    let code = [KeyCode::Char('j'),KeyCode::Char('k'),KeyCode::Char('g'),KeyCode::Char('G'),KeyCode::PageDown,KeyCode::PageUp,KeyCode::Tab,KeyCode::BackTab,KeyCode::Enter,KeyCode::Char('n'),KeyCode::Char('N'),KeyCode::Char('l'),KeyCode::Char('i')][usize::from(which)];
                    let expected_scroll = match which {
                        0 => Some((old_scroll+1).min(max)),
                        1 => Some(old_scroll.saturating_sub(1)),
                        2 => Some(0), 3 => Some(max),
                        4 => Some((old_scroll+page).min(max)),
                        5 => Some(old_scroll.saturating_sub(page)),
                        _ => None,
                    };
                    let expected_active = if (which == 6 || which == 7) && !state.interactive_blocks.is_empty() {
                        let len = state.interactive_blocks.len();
                        Some(match state.active { None => 0, Some(i) => (i + if which == 6 {1} else {len-1}) % len })
                    } else { None };
                    let expected_match = if (which == 9 || which == 10) && !state.search_matches.is_empty() {
                        let len = state.search_matches.len();
                        Some((state.search_current.unwrap() + if which == 9 {1} else {len-1}) % len)
                    } else { None };
                    // `i` flips the active diagram when graphics are on and
                    // never moves the scroll position or the row count.
                    let mut expected_toggled = state.toggled_diagrams.clone();
                    if which == 12 && let Some(bi) = selected(&state) && state.images.is_some() && matches!(state.content[bi],RenderedBlock::Diagram { image: Some(_),.. }) && !expected_toggled.remove(&bi) {
                        expected_toggled.insert(bi);
                    }
                    // A match `n`/`N` lands on inside a diagram image switches
                    // that diagram to ASCII.
                    if let Some(current) = expected_match && let Some(bi) = shown_image_at(&state,state.search_matches[current]) && !expected_toggled.remove(&bi) {
                        expected_toggled.insert(bi);
                    }
                    let old_len = state.flat_lines.len();
                    let mut expected_expanded = state.expanded.clone();
                    if which == 8 && let Some(bi) = selected(&state) && matches!(state.content[bi],RenderedBlock::Diagram {..}) && !expected_expanded.remove(&bi) {
                        expected_expanded.insert(bi);
                    }
                    key(&mut state,code);
                    prop_assert_eq!(&state.expanded, &expected_expanded);
                    prop_assert_eq!(&state.toggled_diagrams, &expected_toggled);
                    if which == 12 {
                        prop_assert_eq!(state.scroll, old_scroll);
                        prop_assert_eq!(state.flat_lines.len(), old_len);
                    }
                    if let Some(scroll) = expected_scroll { prop_assert_eq!(state.scroll,scroll); }
                    if let Some(active) = expected_active { prop_assert_eq!(state.active,Some(active)); }
                    if let Some(current) = expected_match { prop_assert_eq!(state.search_current,Some(current)); }
                }
                PagerOp::Search(query) => {
                    key(&mut state,KeyCode::Char('/'));
                    for c in query.chars() { key(&mut state,KeyCode::Char(c)); check(&state); }
                    key(&mut state,KeyCode::Enter);
                    prop_assert_eq!(&state.search_query,&query);
                }
                PagerOp::Resize(w,h) => {
                    let previous = selected(&state);
                    state.resize(w,h);
                    if let Some(bi) = previous && state.interactive_blocks.iter().any(|e| e.block_index == bi) {
                        prop_assert_eq!(selected(&state),Some(bi));
                    }
                }
                PagerOp::Reload(content) => { state.replace_content(content); }
                PagerOp::ReloadDiagram(input) => {
                    let theme = crate::theme::Theme::default_theme();
                    let mut content = Vec::new();
                    for (source, kind) in [(input.flowchart(crate::strategies::Topology::Cyclic),DiagramKind::Flowchart),(input.sequence(),DiagramKind::Sequence),(input.er(),DiagramKind::Er)] {
                        let diagram = crate::mermaid::render_diagram(&source,theme,usize::from(state.terminal_width)).unwrap();
                        // With graphics on, some diagrams are already rasterized.
                        if let Some(images) = state.images.as_mut() && kind != DiagramKind::Er {
                            images.insert_ready_diagram(&diagram.image);
                        }
                        content.push(RenderedBlock::Diagram { lines: diagram.lines,node_count: diagram.primary_count,edge_count: diagram.secondary_count,kind,image: Some(std::sync::Arc::new(diagram.image)) });
                    }
                    state.replace_content(content);
                }
                PagerOp::ImageToggleTwice => {
                    let before = state.flat_lines.clone();
                    let toggled = state.toggled_diagrams.clone();
                    let scroll = state.scroll;
                    key(&mut state,KeyCode::Char('i')); check(&state);
                    prop_assert_eq!(state.flat_lines.len(),before.len());
                    key(&mut state,KeyCode::Char('i'));
                    prop_assert_eq!(&state.flat_lines,&before);
                    prop_assert_eq!(&state.toggled_diagrams,&toggled);
                    prop_assert_eq!(state.scroll,scroll);
                }
                PagerOp::ToggleTwice => {
                    if let Some(bi) = selected(&state) && matches!(state.content[bi],RenderedBlock::Diagram {..}) {
                        let before = state.flat_lines.clone();
                        let expanded = state.expanded.clone();
                        key(&mut state,KeyCode::Enter); check(&state);
                        key(&mut state,KeyCode::Enter);
                        prop_assert_eq!(&state.flat_lines,&before);
                        prop_assert_eq!(&state.expanded,&expanded);
                    }
                }
            }
            check(&state);
        }
    }
}
