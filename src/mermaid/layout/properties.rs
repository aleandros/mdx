use super::*;
use crate::strategies;
use proptest::prelude::*;

pub(crate) fn check_geometry(result: &LayoutResult) {
    for (i, a) in result.nodes.iter().enumerate() {
        assert!(
            a.x + a.width <= result.width && a.y + a.height <= result.height,
            "out of bounds: {a:?}"
        );
        for b in &result.nodes[i + 1..] {
            assert!(
                a.x + a.width <= b.x
                    || b.x + b.width <= a.x
                    || a.y + a.height <= b.y
                    || b.y + b.height <= a.y,
                "overlap: {a:?}, {b:?}"
            );
        }
    }
    for edge in &result.edges {
        let from = result.nodes.iter().find(|n| n.id == edge.from).unwrap();
        let to = result.nodes.iter().find(|n| n.id == edge.to).unwrap();
        // The source port is the first cell OUTSIDE the box; the arrow tip
        // lies ON the target border (including same-rank and self edges).
        let &(x, y) = edge.points.first().unwrap();
        assert!(
            ((x + 1 == from.x || x == from.x + from.width)
                && y >= from.y
                && y < from.y + from.height)
                || ((y + 1 == from.y || y == from.y + from.height)
                    && x >= from.x
                    && x < from.x + from.width),
            "source port: {edge:?}, {from:?}"
        );
        let &(x, y) = edge.points.last().unwrap();
        assert!(x >= to.x && x < to.x + to.width && y >= to.y && y < to.y + to.height);
        assert!(x == to.x || x + 1 == to.x + to.width || y == to.y || y + 1 == to.y + to.height);
        for &(x, y) in &edge.points {
            assert!(x < result.width && y < result.height);
        }
    }
}
fn check_chart(chart: &FlowChart) {
    let result = layout(chart);
    check_geometry(&result);
    assert_eq!(result.nodes.len(), chart.nodes.len());
    assert_eq!(result.edges.len(), chart.edges.len());
    super::tests::assert_edges_avoid_foreign_nodes(&result, "generated graph");
    let second = layout(chart);
    assert_eq!(format!("{result:?}"), format!("{second:?}"));
    let output = crate::mermaid::ascii::render(&result).join("\n");
    for node in &chart.nodes {
        assert_eq!(
            output.matches(&node.label).count(),
            1,
            "node label {} in {output}",
            node.label
        );
    }
    for edge in &chart.edges {
        if let Some(label) = &edge.label {
            assert_eq!(
                output.matches(label).count(),
                1,
                "edge label {label} in {output}"
            );
        }
    }
}
proptest! {
    #![proptest_config(strategies::model_config(64))]
    #[test]
    fn property_dag_geometry(input in strategies::graph()) {
        let chart = crate::mermaid::parse::parse_flowchart(&input.flowchart(crate::strategies::Topology::Dag)).unwrap();
        check_chart(&chart);
    }
    #[test]
    fn property_cyclic_geometry(input in strategies::graph()) {
        let chart = crate::mermaid::parse::parse_flowchart(&input.flowchart(crate::strategies::Topology::Cyclic)).unwrap();
        check_chart(&chart);
    }
    #[test]
    fn property_er_geometry(input in strategies::graph(), width in strategies::width()) {
        let mut diagram = crate::mermaid::er::parse::parse_er(&input.er()).unwrap();
        let chart = crate::mermaid::er::layout::to_flowchart(&mut diagram,usize::from(width));
        check_chart(&chart);
    }
}
