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

### Roadmap

Things that still need to be implemented (loosely sorted by priority):

- MPQ: Load order of interface MPQs
- Third Person Camera Controller (and sending `MOVE_FACING` packets / reworking the movement tracker)
- hecs:
    - Add more components and unpack update messages further
    - Implement spline walking (NPCs have predefined splines)
    - Rendering thereof
- Debug Shader Reloading
- Command Line Arguments (standalone: map, otherwise: server, credentials)
- Cross Plattform support (i.e. better keybinds, investigate MBP 2011 failure)
- Physics:
    - Collider scaling
    - Character Controller has room for improvements (using the tangent instead of the normals)
    - Offload into a dedicated thread (cpu time rises when colliding, slowing down FPS) -> We need a concept for
      syncing. Currently, we tick at 1/60, but per frame, so wrong.
    - WMO: BSP tree (MOBN, MOBR) for collision meshes instead of the render meshes.
- Reading of DBC files, especially in preparation for:
    - Game Logic. Casting spells and showing stats (mana, health) mainly.
    - Somehow handle locales. We get MPQs from one locale mostly and that locale is the only one filled in DBC strings
- Portals, Water, and other less important render objects
- Audio System, potentially leveraging HRTF and precise reflection and absorption (e.g. SteamAudio)
- UI/Addon System: This will most likely be using mlua and if possible port
  the entirety of "`FrameXML`", so that the original UI code can be run, but for that
  a lot of API surface and especially the related event handling and layout management
  needs to be handled from scratch.
- Advanced game "logic" (e.g. chat, friend list, guilds, trading, auction house)
- Advanced rendering techniques: AO, TAA, VXGI?
- Anisotropic Filtering, basically setting SamplerDesc#anisotropy_clamp > 1, POT, < 16 (based on the device limits)
- https://docs.rs/arc-swap/latest/arc_swap/