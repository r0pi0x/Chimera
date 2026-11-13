//! `chimera_present`'s integration tests, as one test binary (each file a
//! module): spec test cases of the renderer's and audio's CPU side that
//! need nothing above the presentation, and scenario tests of what it
//! draws on the headless harness (`chimera_world::harness::Sim`, the
//! engine's `core` and console as the base: prop LODs, prop shadows,
//! body parts' posed bounds),
//! the view-model camera's switch, a view model drawn without a map,
//! the under-water overlay's,
//! fog-only water's fog and the sky under water; the sound recorded
//! into a WAV file a frame at a time (`audio_record`). `cargo test -p chimera_present --features dev
//! --test it`; where tests go: docs/OBSERVABILITY.md, section 1.

mod audio_record;
mod bc_textures;
mod body_bounds;
mod body_swap;
mod camera_outputs;
mod idle_lights;
mod late_build;
mod particle_depth;
mod phong_spec;
mod map_unload;
mod prop_fade;
mod prop_relight;
mod prop_lods;
mod prop_meshes_ahead;
mod prop_shadows;
mod shader_spec;
mod shadow_spec;
mod underwater_fog_volume;
mod underwater_overlay;
mod underwater_sky;
mod view_model_camera;
mod view_model_without_map;
mod voice_limits;
mod wav_playback;
