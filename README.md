# Sargerust

Sargerust is an attempt at writing a modern, rust-based client for an MMORPG.

For me, the biggest two goals thereby are:

- learning/getting more acquainted with everything involved:
  Rust, WebGPU, modern rendering techniques, older file formats, multithreading and networking
- experimenting to see how far one can get when writing a client from scratch and trying _not_
  to mirror the original very closely. This especially has the rendering in mind, one can use
  much more recent rendering tricks (triplanar mapping, VXGI), potentially some upscaling, to
  make the game look much more recent while keeping the gameplay and appeal the same.

Goals beyond that, as well as completion, are unlikely, because it's a gigantic pile of work
to achieve and because the original client really doesn't leave much to be desired (it even
runs well under wine!).

Now due to the learning nature of the project, feel free to hit me up with suggestions about design,
reviewing my code to suggest more idiomatic rust approaches as well as filling the gaps (I leave
a lot of `// TODO:` comments everywhere I have an idea while coding).

TODO: Watch https://www.youtube.com/watch?v=uEtI7JRBVXk 1h+

## Getting Started

When running the code (through `cargo run`), you have to specify a few options, especially if the defaults don't fit.
Try `cargo run -- --help` to see them. Essentially there are two modes: standalone, which is rendering the world without
a server and "remote", which is the classic mode that connects to an upstream realm- and worldserver.

### Roadmap

Things that still need to be implemented (loosely sorted by priority):

- MPQ: Load order of interface MPQs
- Third Person Camera Controller (and sending `MOVE_FACING` packets / reworking the movement tracker)
- Debug Shader Reloading. NOTE: This requires extensive rend3 changes because we need to reset the ShaderPreProcessor
  and rebuild the base_graph that is usually only built in async_start, once.
- M2: Properly derive whether Alpha Key (Cutoff) shall be used or not, allowing for less texture fetches in shadows
- M2: Cull Modes (Northshire cypress trees), there we'd need to disable culling / duplicate indices.
- Configuration
- CpuDriven support for shaders: That makes it work in non-bindless (binding) mode on old devices
- massive Map Manager rework
- Instanced Rendering of M2s (UnitMaterial is currently created in-place, even if the same texture has already been
  used.) It remains to be seen if that is enough for rend3 to auto instance, though.
    - TODO: Does Rend3 even have instancing?
- Portals, Water, and other less important render objects
- Properly Kill tracy
- hecs:
    - Add more components and unpack update messages further
    - Implement spline walking (NPCs have predefined splines)
    - Rendering thereof
- Physics:
    - Interpolation of Player Position (otherwise stuttery feel)
    - Verify that the colliders are scaled in the right coordinate system (e.g. scaling along z does the right, expected
      thing)
    - M2 Colliders could be derived from the full mesh instead of trying to merge it again
    - Character Controller has room for improvements (using the tangent instead of the normals)
    - WMO: BSP tree (MOBN, MOBR) for collision meshes instead of the render meshes.
    - Terrain Holes such as Ironforge Entrance
- Spell and Stats DBCs
- Audio System, potentially leveraging HRTF and precise reflection and absorption (e.g. SteamAudio)
- UI/Addon System: This will most likely be using mlua and if possible port
  the entirety of "`FrameXML`", so that the original UI code can be run, but for that
  a lot of API surface and especially the related event handling and layout management
  needs to be handled from scratch.
- Advanced game "logic" (e.g. chat, friend list, guilds, trading, auction house)
- Advanced rendering techniques: FXAA+SMAA?, VXGI?
    - HBAO:
        - https://developer.download.nvidia.com/presentations/2008/SIGGRAPH/HBAO_SIG08b.pdf
        - https://developer.download.nvidia.com/assets/gamedev/files/sdk/11/SSAO11.pdf https://github.com/NVIDIAGameWorks/HBAOPlus
        - Visbility Bitmasks as extension (VBAO)
    - MSAA should have alpha-to-coverage
    - Cascaded Shadow Mapping
    - Clustered Forward Rendering
    - GPU Culling?
    - Meshlet?

### Rend3 Fork Ideas

- SPP &mut (already done)
- better panicking in ShaderVertexBufferHelper::generate_template, when VertexAttributes are missing
- Anisotropy of Textures
- Make Material#key() a u64 bitset and & with forward pipelines instead of equaling, which allows some (e.g. depth pre)
  passes to render for all kind of material variants instead of requiring tens of pipelines for those.
- Maybe a way to disable sorting when a depth prepass is done anyway
- Auto Instancing
- Pass label to Texture Descriptor. Hint: Currently not possible without leaking as the InternalTexture requires 'static
  bounds
- Compute Pass Merging in Render Graph