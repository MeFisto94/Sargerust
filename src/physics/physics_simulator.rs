use glam::Vec3;
use itertools::Itertools;
use rapier3d::control::{EffectiveCharacterMovement, KinematicCharacterController};
use rapier3d::prelude::*;

pub struct PhysicsSimulator {
    integration_parameters: IntegrationParameters,
    physics_pipeline: PhysicsPipeline,
    island_manager: IslandManager,
    ccd_solver: CCDSolver,
    broad_phase: BroadPhaseMultiSap,
    narrow_phase: NarrowPhase,
    gravity: Vector<Real>,
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    physics_hooks: (),
    event_handler: (),
    queries: QueryPipeline,
}

impl Default for PhysicsSimulator {
    fn default() -> Self {
        Self {
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: BroadPhaseMultiSap::new(),
            narrow_phase: NarrowPhase::new(),
            // impulse and multibody joint set.
            ccd_solver: CCDSolver::new(),
            // TODO: gravity should be in foot per quarterpounder ;)
            gravity: vector![0.0, 0.0, -9.81], // We're the z-up blender coordinate system.
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            physics_hooks: (),
            event_handler: (),
            queries: QueryPipeline::new(),
        }
    }
}

impl PhysicsSimulator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn step(&mut self) {
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            Some(&mut self.queries),
            &self.physics_hooks,
            &self.event_handler,
        );
    }

    pub fn insert_collider(&mut self, collider: Collider) -> ColliderHandle {
        self.collider_set.insert(collider)
    }

    pub fn insert_colliders(
        &mut self,
        colliders: Vec<Collider>,
        parent_handle: RigidBodyHandle,
    ) -> Vec<ColliderHandle> {
        colliders
            .into_iter()
            .map(|collider| {
                self.collider_set
                    .insert_with_parent(collider, parent_handle, &mut self.rigid_body_set)
            })
            .collect_vec()
    }

    pub fn drop_collider(&mut self, collider: ColliderHandle, wake_up: bool) {
        self.collider_set.remove(
            collider,
            &mut self.island_manager,
            &mut self.rigid_body_set,
            wake_up,
        );
    }

    pub fn insert_rigid_body(&mut self, rigid_body: RigidBody) -> RigidBodyHandle {
        self.rigid_body_set.insert(rigid_body)
    }

    pub fn get_rigidbody(&self, rigid_body_handle: RigidBodyHandle) -> Option<&RigidBody> {
        self.rigid_body_set.get(rigid_body_handle)
    }

    pub fn teleport_collider(&mut self, collider: ColliderHandle, translation: Vec3) {
        self.collider_set
            .get_mut(collider)
            .expect("Collider to be present")
            .set_translation(translation.into());
    }

    pub fn move_character(
        &mut self,
        controller: &KinematicCharacterController,
        collider_handle: ColliderHandle,
        mass: f32,
        desired_translation: Vec3,
    ) -> EffectiveCharacterMovement {
        let mut collisions = vec![];
        let collider = self
            .collider_set
            .get(collider_handle)
            .expect("Collider Handle to be valid");

        let movement = controller.move_shape(
            self.integration_parameters.dt,
            &self.rigid_body_set,
            &self.collider_set,
            &self.queries,
            collider.shape(),
            collider.position(),
            desired_translation.into(),
            QueryFilter::default().exclude_collider(collider_handle),
            |collision| collisions.push(collision),
        );

        // kick away other objects.
        controller.solve_character_collision_impulses(
            self.integration_parameters.dt,
            &mut self.rigid_body_set,
            &self.collider_set,
            &self.queries,
            collider.shape(),
            mass,
            &collisions,
            QueryFilter::default().exclude_collider(collider_handle),
        );

        movement
    }
}
