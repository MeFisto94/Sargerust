use crate::entity::components::objects::{SplineWalker, TmpLocation};
use crate::game::application::GameApplication;
use crate::networking::utils::net_vector3d_to_glam;
use log::{debug, info, trace};

#[derive(Default)]
pub struct SplineWalkerSystem {}

impl SplineWalkerSystem {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn update(&self, app: &GameApplication, delta_time: f32) {
        let mut write = app
            .entity_tracker
            .world()
            .write()
            .expect("World Write Lock poisoned");

        let mut finished_entities = vec![];

        for (entity, (spline_walker, location)) in write.query_mut::<(&mut SplineWalker, &mut TmpLocation)>() {
            spline_walker.time_passed = (spline_walker.time_passed + delta_time).min(spline_walker.duration);

            let spline_progress = spline_walker.time_passed / spline_walker.duration;
            let nodes_progress = spline_walker.nodes.len() as f32 * spline_progress;

            let node_index = nodes_progress as usize;
            let intra_node_progress = nodes_progress - node_index as f32; // basically the remainder of the float.

            // only if we're exactly at the end of the spline, we'd out of bounds, thus:
            if node_index >= spline_walker.nodes.len() - 1 {
                if spline_walker.flags.get_cyclic() {
                    debug!(
                        "Spline reached the end at {}, looping.",
                        spline_walker.time_passed
                    );
                    // TODO: Just setting time_passed to 0 will fail because it will assume we are at the start. This
                    //  only works if start == end.
                    spline_walker.time_passed = 0.0;

                    if spline_walker.flags.get_enter_cycle() {
                        // TODO: This seems to mean that we remove the first node after the first full cycle.
                    }
                } else {
                    debug!("Spline is not cyclic, removing.");
                    finished_entities.push(entity);
                    location.0 = net_vector3d_to_glam(spline_walker.nodes[spline_walker.nodes.len() - 1]);
                }

                continue;
            }

            let node = net_vector3d_to_glam(spline_walker.nodes[node_index]);
            let target_node = net_vector3d_to_glam(spline_walker.nodes[node_index + 1]);

            location.0 = node.lerp(target_node, intra_node_progress);
        }

        for entity in finished_entities {
            write
                .remove_one::<SplineWalker>(entity)
                .expect("Removing SplineWalker");
        }
    }
}
