use framexml_parser::dimensions::SizeType;
use framexml_parser::typedefs::LayoutFrameType;

#[derive(Debug, Copy, Clone)]
pub struct LayoutedFrame {
    pub position: glam::Vec2,
    pub size: glam::Vec2,
}

pub struct LayoutManager {}

impl LayoutManager {
    // TODO: once Frame has all relevant info, get rid of LayoutFrameType which is just the "DTO"
    pub fn layout_frame(&self, frame: &LayoutFrameType, parent_layout: &LayoutedFrame) -> Result<LayoutedFrame, ()> {
        let size = frame
            .size
            .as_ref()
            .map(|size| self.layout_size(size, parent_layout))
            .unwrap_or(glam::Vec2::ZERO);

        frame.anchors.as_ref().unwrap().elements.get(0).unwrap();

        Ok(LayoutedFrame {
            size,
            position: glam::Vec2::ZERO,
        })
    }

    // https://wowwiki-archive.fandom.com/wiki/XML/LayoutFrame
    fn layout_size(&self, size: &SizeType, parent_layout: &LayoutedFrame) -> glam::Vec2 {
        // TODO: non-absolute sizes?

        if size.x.is_none() || size.y.is_none() {
            unimplemented!("Relative sizes are not implemented yet");
            // They will probably read the non abs-dimensions variants of size, but even those seem to mean a different thing
        }

        if !size.dimensions.is_empty() {
            unimplemented!("Dimensions are not implemented yet");
        }

        glam::Vec2::new(size.x.unwrap() as f32, size.y.unwrap() as f32)
    }
}
