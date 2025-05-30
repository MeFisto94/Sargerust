use crate::entity::components::objects::{SplineWalker, TmpLocation, TmpOrientation};
use crate::entity::components::rendering::Renderable;
use crate::entity::components::units::UnitDisplayId;
use crate::networking::utils::net_vector3d_to_glam;
use glam::Vec3;
use hecs::World;
use itertools::Itertools;
use log::{debug, info, warn};
use std::sync::RwLock;
use wow_world_messages::Guid;
use wow_world_messages::wrath::{
    MovementBlock, MovementBlock_UpdateFlag_Living, Object, ObjectType, SMSG_MONSTER_MOVE, UpdateMask, Vector3d,
};

// TODO: remove at some point, when we know more about the packet values.
#[allow(unused_variables)]
#[derive(Default)]
pub struct EntityTracker {
    world: RwLock<World>,
}

impl EntityTracker {
    pub fn new() -> Self {
        EntityTracker::default()
    }

    pub fn world(&self) -> &RwLock<World> {
        &self.world
    }

    // TODO: Make this async so we can fire and forget from the packet handler, not stalling on locks?
    pub fn update_objects(&self, objects: &[Object]) {
        for object in objects {
            match object {
                Object::CreateObject {
                    guid3: guid,
                    mask2: mask,
                    movement2: movement,
                    object_type,
                } => self.create_object(guid, mask, movement, object_type, false),
                Object::CreateObject2 {
                    guid3: guid,
                    mask2: mask,
                    movement2: movement,
                    object_type,
                } => self.create_object(guid, mask, movement, object_type, true),
                Object::OutOfRangeObjects { guids } => self.destroy_objects(guids),
                Object::NearObjects { guids } => self.destroy_objects(guids),
                Object::Movement { guid2, movement1 } => self.update_object_movement(guid2, movement1),
                Object::Values { guid1, mask1 } => self.update_object_values(guid1, mask1),
            };
        }
    }

    fn create_object(
        &self,
        guid: &Guid,
        mask: &UpdateMask,
        movement: &MovementBlock,
        object_type: &ObjectType,
        is_two: bool,
    ) {
        {
            let mut world = self.world.write().expect("World Write Lock");
            let pos_rot = Self::movement_block_pos_rot(movement);

            let entity = world.spawn((*guid, *object_type));

            if let Some((position, orientation)) = pos_rot {
                world
                    .insert(
                        entity,
                        (
                            TmpLocation(net_vector3d_to_glam(position)),
                            TmpOrientation(orientation),
                        ),
                    )
                    .expect("Insert Position and Orientation");
            }

            if let Some(MovementBlock_UpdateFlag_Living::Living { flags, .. }) = movement.update_flag.get_living() {
                if let Some(spline) = flags.get_spline_enabled() {
                    world
                        .insert_one(entity, SplineWalker::from(spline))
                        .expect("Insert SplineWalker");
                }
            };

            match mask {
                UpdateMask::GameObject(_) => (), // Game objects don't seem to have anything useful for us at the moment
                UpdateMask::Unit(unit) => {
                    let level = unit.unit_level().expect("Unit Level to be mandatory");
                    world.insert_one(entity, level).expect("Insert Level");

                    if let Some(display_id) = unit.unit_displayid() {
                        world
                            .insert_one(entity, UnitDisplayId(display_id))
                            .expect("Insert DisplayId");
                    }
                }
                UpdateMask::Player(player) => {
                    let level = player.unit_level().expect("Unit Level to be mandatory");
                    world.insert_one(entity, level).expect("Insert Level");

                    debug!("level: {:?}", level);
                    debug!("player-unit: {:?}", player.unit_bytes_0());
                    world
                        .insert_one(entity, Renderable::default())
                        .expect("Insert Renderable");
                }
                _ => info!("Ignoring UpdateMask {:?}", mask),
            };
        }
    }

    fn update_object_movement(&self, guid: &Guid, movement_block: &MovementBlock) {
        // TODO: how does this differ from monster_move?
        let mut write = self.world.write().expect("World Write Lock");
        let entity = write
            .query_mut::<(&Guid, &mut TmpLocation, &mut TmpOrientation)>()
            .into_iter()
            .find(|(_, (&entity_guid, _, _))| entity_guid == *guid);

        if entity.is_none() {
            warn!(
                "Could not update object with GUID {:?}, because it wasn't known to us",
                guid
            );

            return;
        }

        let (_, (_, location, orientation)) = entity.unwrap();
        if let Some((position, rotation)) = Self::movement_block_pos_rot(movement_block) {
            debug!("Updating position and orientation for {:?}", guid);
            location.0 = Vec3::new(position.x, position.y, position.z);
            orientation.0 = rotation;
        }
    }

    pub fn move_monster(&self, msg: &SMSG_MONSTER_MOVE) {
        return; // TODO: Implement - better -

        let mut write = self.world.write().expect("World Write Lock");
        let entity = write
            .query_mut::<(&Guid, &mut SplineWalker)>()
            .into_iter()
            .find(|(_, (&entity_guid, _))| entity_guid == msg.guid);

        if let Some((_, (_, spline_walker))) = entity {
            spline_walker.update_from(msg);
        } else {
            let query_result = write
                .query_mut::<&Guid>()
                .into_iter()
                .find(|(_, &guid)| guid == msg.guid);

            if let Some((entity, _)) = query_result {
                write
                    .insert_one(entity, SplineWalker::from(msg))
                    .expect("Insert SplineWalker");
            } else {
                warn!("Couldn't find any entity with GUID {:?}", msg.guid);
            }
        };
    }

    fn update_object_values(&self, guid: &Guid, update_mask: &UpdateMask) {
        info!("Update Object Values for {} not implemented yet", guid);
    }

    fn movement_block_pos_rot(movement: &MovementBlock) -> Option<(Vector3d, f32)> {
        movement
            .update_flag
            .get_living()
            .map(|living| match living {
                MovementBlock_UpdateFlag_Living::Living {
                    position,
                    orientation,
                    ..
                } => (*position, *orientation),
                MovementBlock_UpdateFlag_Living::Position {
                    position1,
                    orientation1,
                    ..
                } => (*position1, *orientation1),
                MovementBlock_UpdateFlag_Living::HasPosition {
                    orientation2,
                    position2,
                } => (*position2, *orientation2),
            })
    }

    pub fn destroy_object(&self, guid: Guid, target_died: bool) {
        let mut world = self.world.write().expect("World Read Lock");
        let entity = world
            .query_mut::<&Guid>()
            .into_iter()
            .find(|(_, &entity_guid)| guid == entity_guid);
        if let Some((id, _)) = entity {
            world
                .despawn(id)
                .expect("We just found the entity, it has to exist");
        } else {
            warn!(
                "Could not destroy object with GUID {:?}, because it wasn't known to us",
                guid
            );
        }
    }

    fn destroy_objects(&self, guids: &[Guid]) {
        let mut world = self.world.write().expect("World Read Lock");
        let entities = world
            .query_mut::<&Guid>()
            .into_iter()
            .filter(|(_, &entity_guid)| guids.contains(&entity_guid))
            .map(|(id, _)| id)
            .collect_vec();

        for entity in entities {
            world
                .despawn(entity)
                .expect("We just found the entity, it has to exist");
        }
    }
}
