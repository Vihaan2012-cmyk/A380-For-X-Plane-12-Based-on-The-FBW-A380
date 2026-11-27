//! FlyByWire's cockpit glTF drives both sidesticks from one pair of clips, so the F/O stick followed the
//! captain's. Its behaviour XML names the F/O stick's own clips; this splits the shared clips to match.

use crate::model::glb::Model;

#[derive(Debug, Clone, PartialEq)]
pub struct Stick {
    pub node: String,
    pub x: String,
    pub y: String,
}

/// Every `A32NX_Sidestick_Template` use in a behaviour XML, in order.
pub fn template_sticks(xml: &str) -> Vec<Stick> {
    let tag = |block: &str, name: &str| -> Option<String> {
        let open = format!("<{name}>");
        let start = block.find(&open)? + open.len();
        let end = block[start..].find(&format!("</{name}>"))? + start;
        Some(block[start..end].trim().to_string())
    };
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(at) = rest.find("Name=\"A32NX_Sidestick_Template\"") {
        let tail = &rest[at..];
        let end = tail.find("</UseTemplate>").unwrap_or(tail.len());
        let block = &tail[..end];
        if let (Some(node), Some(x), Some(y)) = (tag(block, "NODE_ID"), tag(block, "ANIM_NAME_YOKE_X"), tag(block, "ANIM_NAME_YOKE_Y")) {
            out.push(Stick { node, x, y });
        }
        rest = &tail[end..];
    }
    out
}

fn ancestors(model: &Model, node: usize) -> Vec<usize> {
    let mut out = vec![node];
    let mut n = model.nodes[node].parent;
    while let Some(k) = n {
        if out.len() > 128 {
            break;
        }
        out.push(k);
        n = model.nodes[k].parent;
    }
    out
}

/// Whether `n` lies on `stick`'s own branch: an ancestor or descendant of it that is not shared with `other`.
fn on_branch(model: &Model, n: usize, stick: usize, other: usize) -> bool {
    let up = ancestors(model, stick);
    let shared = ancestors(model, other);
    let related = up.contains(&n) || ancestors(model, n).contains(&stick);
    related && !shared.contains(&n) && !ancestors(model, n).contains(&other)
}

/// Moves the channels of each named-but-missing stick clip out of the clip the first stick uses.
pub fn split_shared(model: &mut Model, sticks: &[Stick]) -> Vec<String> {
    let find_node = |m: &Model, name: &str| m.nodes.iter().position(|n| n.name == name);
    let has_clip = |m: &Model, name: &str| m.clips.iter().any(|c| c.name == name);
    let mut report = Vec::new();
    let Some(source) = sticks.iter().find(|s| has_clip(model, &s.x) && has_clip(model, &s.y)) else { return report };
    let Some(source_node) = find_node(model, &source.node) else { return report };
    for stick in sticks.iter().filter(|s| *s != source) {
        let Some(node) = find_node(model, &stick.node) else { continue };
        for (from, to) in [(&source.x, &stick.x), (&source.y, &stick.y)] {
            if has_clip(model, to) {
                continue;
            }
            let Some(ci) = model.clips.iter().position(|c| &c.name == from) else { continue };
            let channels = std::mem::take(&mut model.clips[ci].channels);
            let (moved, kept): (Vec<_>, Vec<_>) = channels.into_iter().partition(|ch| on_branch(model, ch.node, node, source_node));
            model.clips[ci].channels = kept;
            if moved.is_empty() {
                continue;
            }
            report.push(format!("sidesticks: {} channel(s) of {from} drive {}; now their own clip {to}", moved.len(), stick.node));
            model.clips.push(crate::model::glb::Clip { name: to.clone(), channels: moved });
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::glb::{AnimPath, Channel, Clip, Node};

    fn node(name: &str, parent: Option<usize>) -> Node {
        Node { name: name.into(), parent, translation: [0.; 3], rotation: [0., 0., 0., 1.], scale: [1.; 3], world: [0.; 16] }
    }

    fn channel(node: usize) -> Channel {
        Channel { node, path: AnimPath::Rotation, times: vec![0., 1.], values: vec![[0.; 4]; 2], step: false }
    }

    const XML: &str = r#"
        <UseTemplate Name="A32NX_Sidestick_Template">
            <NODE_ID>YOKE_LEFT</NODE_ID>
            <ANIM_NAME_YOKE_X>yoke_lever_stick1_l_r</ANIM_NAME_YOKE_X>
            <ANIM_NAME_YOKE_Y>yoke_lever_stick_fore1_aft</ANIM_NAME_YOKE_Y>
        </UseTemplate>
        <UseTemplate Name="A32NX_Sidestick_Template">
            <NODE_ID>YOKE_RIGHT</NODE_ID>
            <ANIM_NAME_YOKE_X>yoke_lever_stick2_l_r</ANIM_NAME_YOKE_X>
            <ANIM_NAME_YOKE_Y>yoke_lever_stick_fore2_aft</ANIM_NAME_YOKE_Y>
        </UseTemplate>"#;

    #[test]
    fn the_behaviour_names_both_sticks() {
        let s = template_sticks(XML);
        assert_eq!(s.len(), 2);
        assert_eq!(s[1], Stick { node: "YOKE_RIGHT".into(), x: "yoke_lever_stick2_l_r".into(), y: "yoke_lever_stick_fore2_aft".into() });
    }

    #[test]
    fn the_fo_stick_gets_its_own_clips() {
        // CENTER <- L_DUMMY2 <- L_DUMMY1 <- YOKE_LEFT, and the same on the right, as in FlyByWire's glTF.
        let mut m = Model {
            nodes: vec![
                node("CENTER", None),
                node("YOKE_LEFT_DUMMY2", Some(0)),
                node("YOKE_LEFT_DUMMY1", Some(1)),
                node("YOKE_LEFT", Some(2)),
                node("YOKE_RIGHT_DUMMY2", Some(0)),
                node("YOKE_RIGHT_DUMMY1", Some(4)),
                node("YOKE_RIGHT", Some(5)),
            ],
            ..Default::default()
        };
        m.clips = vec![
            Clip { name: "yoke_lever_stick1_l_r".into(), channels: vec![channel(2), channel(5)] },
            Clip { name: "yoke_lever_stick_fore1_aft".into(), channels: vec![channel(3), channel(2), channel(5)] },
        ];
        let report = split_shared(&mut m, &template_sticks(XML));
        assert_eq!(report.len(), 2, "{report:?}");
        let nodes = |name: &str| m.clips.iter().find(|c| c.name == name).map(|c| c.channels.iter().map(|ch| ch.node).collect::<Vec<_>>());
        assert_eq!(nodes("yoke_lever_stick1_l_r"), Some(vec![2]));
        assert_eq!(nodes("yoke_lever_stick_fore1_aft"), Some(vec![3, 2]));
        assert_eq!(nodes("yoke_lever_stick2_l_r"), Some(vec![5]));
        assert_eq!(nodes("yoke_lever_stick_fore2_aft"), Some(vec![5]));
        assert!(split_shared(&mut m, &template_sticks(XML)).is_empty(), "runs once");
    }
}
