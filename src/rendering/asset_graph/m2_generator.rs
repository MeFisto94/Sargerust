use std::sync::{Arc, RwLock};

use crate::io::mpq::loader::MPQLoader;
use crate::rendering::asset_graph::nodes::adt_node::{IRTexture, M2Node, WMOGroupNode, WMONode};
use crate::rendering::asset_graph::resolver::GraphNodeGenerator;
use crate::rendering::importer::wmo_importer::WMOGroupImporter;
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
        let dynamic_tex_references = m2.dynamic_textures;

        Arc::new(M2Node {
            tex_reference,
            dynamic_tex_references,
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
        Arc::new(WMOLoader::load_graph(&self.mpq_loader, name).expect("WMO to parse correctly"))
    }
}

impl GraphNodeGenerator<WMOGroupNode> for M2Generator {
    fn generate(&self, name: &str) -> Arc<WMOGroupNode> {
        Arc::new(WMOGroupImporter::load_wmo_group(&self.mpq_loader, name))
    }
}
