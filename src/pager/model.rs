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
fn check(state: &PagerState) {
    assert!(state.scroll <= state.max_scroll());
    if let Some(i) = state.active {
        assert!(i < state.interactive_blocks.len());
        assert!(matches!(
            state.content[state.interactive_blocks[i].block_index],
            RenderedBlock::Diagram { .. } | RenderedBlock::Image { .. }
        ));
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
    fn property_pager_model(content in strategies::rendered_blocks(), ops in strategies::pager_ops(), width in strategies::width()) {
        let mut state = PagerState::new(content,20,width,100,crate::theme::Theme::default_theme());
        // Activating image placeholders must never spawn a desktop application.
        state.opener = None;
        check(&state);
        for op in ops {
            match op {
                PagerOp::Key(which) => {
                    let old_scroll = state.scroll;
                    let max = state.flat_lines.len().saturating_sub(state.terminal_height as usize);
                    let page = usize::from(state.terminal_height).saturating_sub(1).max(1);
                    let code = [KeyCode::Char('j'),KeyCode::Char('k'),KeyCode::Char('g'),KeyCode::Char('G'),KeyCode::PageDown,KeyCode::PageUp,KeyCode::Tab,KeyCode::BackTab,KeyCode::Enter,KeyCode::Char('n'),KeyCode::Char('N'),KeyCode::Char('l')][which as usize];
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
                    let mut expected_expanded = state.expanded.clone();
                    if which == 8 && let Some(bi) = selected(&state) && matches!(state.content[bi],RenderedBlock::Diagram {..}) && !expected_expanded.remove(&bi) {
                        expected_expanded.insert(bi);
                    }
                    key(&mut state,code);
                    prop_assert_eq!(&state.expanded, &expected_expanded);
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
                    for (source, kind) in [(input.flowchart(false),DiagramKind::Flowchart),(input.sequence(),DiagramKind::Sequence),(input.er(),DiagramKind::Er)] {
                        let (lines,node_count,edge_count) = crate::mermaid::render_mermaid(&source,theme,state.terminal_width as usize).unwrap();
                        content.push(RenderedBlock::Diagram { lines,node_count,edge_count,kind });
                    }
                    state.replace_content(content);
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
