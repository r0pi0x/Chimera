//! Dust motes (`func_dustmotes`) and dust clouds (`func_dustcloud`) from
//! specs/cs_source/sprites_dust.md 7-9: the entity keys and its brush
//! model's bounds and brushes become a `MapDust` volume. A cloud is the
//! same entity without the screen-constant size: its Size keys are world
//! half-widths (de_train's two clouds, turned on as the bomb goes off).
//!
//! Known shortcuts (docs/tech-debt.md): Dust motes and clouds (func_dustmotes,
//! func_dustcloud: the same entity with world-sized motes, sprites_dust.md 7;
//! de_train's two after the bomb) spawn inside their brushes (10 tries) and
//! frozen ones once; no wind entities yet (dust2 has none); the material of a
//! cloud is the motes' `particle/sparkles` (the spec's "same entity"). Why it's
//! OK for now: Matches the spec on dust2; de_train's clouds unchecked against
//! CS:S.

use bevy::prelude::*;
use vbsp::Bsp;

use super::{
    bsp::{METERS_PER_UNIT, to_engine},
    material::MaterialLoader,
};
use crate::map::{MapData, MapDust};

/// Entity keys, ignoring case (keys may come back lower-cased).
fn key<'a>(e: &vbsp::RawEntity<'a>, name: &str) -> Option<&'a str> {
    e.properties()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v)
}

pub fn add_dust(bsp: &Bsp, leaves: &[super::ambient::RawLeaf], materials: &mut MaterialLoader, data: &mut MapData) {
    let models: Vec<_> = bsp.models().collect();
    let mut texture = None;
    for (index, e) in bsp
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e.prop("classname"), Some("func_dustmotes" | "func_dustcloud")))
    {
        let cloud = e.prop("classname") == Some("func_dustcloud");
        // StartDisabled: the logic can turn it on later.
        let start_on = !key(&e, "StartDisabled").is_some_and(|v| v.trim_start().starts_with('1'));
        let Some((model_index, model)) = e
            .prop("model")
            .and_then(|m| m.strip_prefix('*'))
            .and_then(|i| i.parse::<usize>().ok())
            .and_then(|i| Some((i, models.get(i)?)))
        else {
            continue;
        };
        let num = |k: &str, d: f32| key(&e, k).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(d);
        // No Alpha key: the motes are invisible.
        let Some(alpha) = key(&e, "Alpha").and_then(|v| v.trim().parse::<f32>().ok()) else {
            continue;
        };
        let rgb: Vec<f32> = e
            .prop("color")
            .unwrap_or("255 255 255")
            .split_whitespace()
            .filter_map(|v| v.parse::<f32>().ok())
            .collect();
        if texture.is_none() {
            texture = materials.resolve("particle/sparkles").texture;
        }
        let (a, b) = (to_engine(model.mins), to_engine(model.maxs));
        data.source_fx_mut().dust.push(MapDust {
            min: a.min(b),
            max: a.max(b),
            rate: num("SpawnRate", 40.0),
            // A cloud's sizes are half-widths in units.
            size: if cloud {
                (num("SizeMin", 10.0) * METERS_PER_UNIT, num("SizeMax", 20.0) * METERS_PER_UNIT)
            } else {
                (num("SizeMin", 10.0), num("SizeMax", 20.0))
            },
            world_size: cloud,
            frozen: key(&e, "Frozen").is_some_and(|v| v.trim_start().starts_with('1')),
            brushes: super::bsp::model_volumes(bsp, leaves, model_index),
            speed: num("SpeedMax", 10.0) * METERS_PER_UNIT,
            // Lifetimes are sent in 4 bits: 0-15 s.
            life: (
                num("LifetimeMin", 5.0).clamp(0.0, 15.0),
                num("LifetimeMax", 10.0).clamp(0.0, 15.0),
            ),
            fade_distance: num("DistMax", 1024.0) * METERS_PER_UNIT,
            color: [
                rgb.first().copied().unwrap_or(255.0) / 255.0,
                rgb.get(1).copied().unwrap_or(255.0) / 255.0,
                rgb.get(2).copied().unwrap_or(255.0) / 255.0,
                alpha / 255.0,
            ],
            texture,
            entity: Some(index),
            start_on,
        });
    }
}
