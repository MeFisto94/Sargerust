use std::sync::{Arc, RwLock};

use crate::io::mpq::loader::MPQLoader;
use crate::rendering::asset_graph::nodes::adt_node::{DoodadReference, IRTexture, M2Node, WMONode};
use crate::rendering::asset_graph::resolver::GraphNodeGenerator;
use crate::rendering::loader::blp_loader::BLPLoader;
use crate::rendering::loader::m2_loader::M2Loader;
use crate::rendering::loader::wmo_loader::WMOLoader;

pub struct M2Generator {
    mpq_loader: Arc<MPQLoader>,
}

impl M2Generator {
    pub fn new(mpq_loader: Arc<MPQLoader>) -> Self {
        Self { mpq_loader }
    }
}

impl GraphNodeGenerator<M2Node> for M2Generator {
    fn generate(&self, name: &str) -> Arc<M2Node> {
        let m2 = M2Loader::load_no_lod_for_graph(&self.mpq_loader, name);
        let mesh = RwLock::new(m2.mesh.into());
        let material = RwLock::new(m2.material.into());
        let tex_reference = m2.textures;

        Arc::new(M2Node {
            tex_reference,
            mesh,
            material,
        })
    }
}

impl GraphNodeGenerator<RwLock<Option<IRTexture>>> for M2Generator {
    fn generate(&self, name: &str) -> Arc<RwLock<Option<IRTexture>>> {
        // TODO: textures are the only one that are allowed to fail? feature request..
        Arc::new(RwLock::new(
            BLPLoader::load_blp_from_ldr(&self.mpq_loader, name).map(|data| IRTexture { data, handle: None }),
        ))
    }
}

impl GraphNodeGenerator<WMONode> for M2Generator {
    fn generate(&self, name: &str) -> Arc<WMONode> {
        let wmo = WMOLoader::load(&self.mpq_loader, name).expect("WMO to parse correctly");
        let mut doodads = Vec::new();

        // TODO: fill from wmo, but do NOT load them with the WMOLoader.
        let mut subgroups = Vec::new();

        for dad in wmo.doodads {
            doodads.push(DoodadReference::new(dad.transform.into(), dad.m2_ref));
        }

        Arc::new(WMONode { doodads, subgroups })
    }
}
