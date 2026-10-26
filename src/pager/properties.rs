use super::*;
use crate::strategies::{self, styled_chars};
use proptest::prelude::*;

proptest! {
    #![proptest_config(strategies::model_config(64))]
    #[test]
    fn property_wrap_preserves_text_and_styles(line in strategies::styled_line(), width in strategies::width()) {
        let wrapped = wrap_styled_line(&line,usize::from(width));
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
        prop_assert_eq!(&wrapped, &wrap_styled_line(&line,usize::from(width)));
    }
    #[test]
    fn property_flatten_preserves_block_order(content in strategies::rendered_blocks(), width in strategies::width(), height in 0u16..50, image_px in prop::option::of((1u32..3000,1u32..3000))) {
        let mut pager = PagerState::new(content,height,width,100,crate::theme::Theme::default_theme());
        // With graphics on, the strategies' `test.png` is a decoded image.
        let image_id = image_px.map(|px| {
            let mut store = crate::images::ImageStore::for_tests("/docs".into(),(10,20));
            let id = store.insert_ready("test.png",px);
            pager.images = Some(store);
            id
        });
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
                        // One placeholder row per reserved row, then the caption.
                        let max_cols = u16::try_from(pager.content_width).unwrap();
                        let max_rows = u16::try_from(u32::from(height)*3/5).unwrap().max(1);
                        if let (Some(id),Some(px)) = (image_id,image_px)
                            && let Some((cols,rows)) = crate::kitty::fit_cells(px,(10,20),max_cols,max_rows)
                        {
                            prop_assert!(cols <= max_cols && rows <= max_rows);
                            for row in 0..rows {
                                prop_assert_eq!(&pager.flat_lines[cursor], &FlatLine::ImageRow { block_index:bi,id,row,cols });
                                cursor += 1;
                            }
                        }
                        prop_assert_eq!(&pager.flat_lines[cursor], &FlatLine::ImagePlaceholder { alt:alt.clone(),url:url.clone(),block_index:bi });
                        cursor += 1;
                        let entry = pager.interactive_blocks.iter().find(|e| e.block_index == bi).unwrap();
                        prop_assert_eq!((entry.flat_line_index,entry.flat_line_end),(start,cursor));
                    }
                    RenderedBlock::Diagram { lines,.. } => {
                        prop_assert_eq!(&pager.flat_lines[cursor], &FlatLine::Styled(StyledLine::empty())); cursor += 1;
                        let large = lines.len() > usize::from(height) || lines.iter().map(line_width).max().unwrap_or(0) > usize::from(width) * 2;
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
