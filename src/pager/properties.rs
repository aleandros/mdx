use super::*;
use crate::strategies::{self, styled_chars};
use proptest::prelude::*;

proptest! {
    #![proptest_config(strategies::model_config(64))]
    #[test]
    fn property_wrap_preserves_text_and_styles(line in strategies::styled_line(), width in strategies::width()) {
        let wrapped = wrap_styled_line(&line,width as usize);
        prop_assert!(!wrapped.is_empty());
        let expected: Vec<_> = styled_chars(&line).into_iter().filter(|(c,_)| !c.is_whitespace()).collect();
        let actual: Vec<_> = wrapped.iter().flat_map(styled_chars).filter(|(c,_)| !c.is_whitespace()).collect();
        prop_assert_eq!(actual, expected);
        for row in &wrapped {
            // A two-cell glyph is indivisible on a one-cell terminal.
            let wide_glyph_alone = width == 1 && styled_chars(row).iter()
                .filter(|(c,_)| UnicodeWidthChar::width(*c).unwrap_or(0) > 0).count() == 1;
            prop_assert!(line_width(row) <= usize::from(width) || wide_glyph_alone, "{row:?}, width={width}");
        }
        prop_assert_eq!(&wrapped, &wrap_styled_line(&line,width as usize));
    }
    #[test]
    fn property_flatten_preserves_block_order(content in strategies::rendered_blocks(), width in strategies::width(), height in 0u16..50) {
        let mut pager = PagerState::new(content,height,width,100,crate::theme::Theme::default_theme());
        for expanded in [false,true] {
            pager.expanded = if expanded { (0..pager.content.len()).collect() } else { HashSet::new() };
            pager.rebuild_flat_lines();
            let mut cursor = 0;
            for (bi,block) in pager.content.iter().enumerate() {
                let start = cursor;
                match block {
                    RenderedBlock::Lines(lines) => {
                        if lines.is_empty() { cursor += 1; }
                        for line in lines {
                            for expected in wrap_styled_line(line,pager.content_width) {
                                prop_assert_eq!(&pager.flat_lines[cursor], &FlatLine::Styled(expected));
                                cursor += 1;
                            }
                        }
                    }
                    RenderedBlock::Image { alt,url } => {
                        prop_assert_eq!(&pager.flat_lines[cursor], &FlatLine::ImagePlaceholder { alt:alt.clone(),url:url.clone(),block_index:bi });
                        cursor += 1;
                    }
                    RenderedBlock::Diagram { lines,.. } => {
                        prop_assert_eq!(&pager.flat_lines[cursor], &FlatLine::Styled(StyledLine::empty())); cursor += 1;
                        let large = lines.len() > height as usize || lines.iter().map(line_width).max().unwrap_or(0) > width as usize * 2;
                        if large && !expanded {
                            prop_assert!(matches!(pager.flat_lines[cursor],FlatLine::DiagramCollapsed { block_index,.. } if block_index == bi), "collapsed block index");
                            cursor += 1;
                        } else {
                            for expected in lines {
                                match &pager.flat_lines[cursor] {
                                    FlatLine::DiagramAscii { line,indent } => { prop_assert_eq!(line,expected); prop_assert!(*indent <= pager.gutter); }
                                    other => prop_assert!(false,"expected ASCII row, got {other:?}"),
                                }
                                cursor += 1;
                            }
                        }
                        prop_assert_eq!(&pager.flat_lines[cursor], &FlatLine::Styled(StyledLine::empty())); cursor += 1;
                    }
                }
                prop_assert!(cursor > start);
            }
            prop_assert_eq!(cursor,pager.flat_lines.len());
        }
    }
}
