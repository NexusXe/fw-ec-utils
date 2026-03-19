use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

use crate::{
    fan_curve::{
        FanProfile,
        curve_lut_gen::generate_fan_curve_lut_dyn,
        curve_parsing::{ParsedCurve, squash_curvedef},
    },
    info, infov, warn,
};

const FAN_CACHE_DIR: &str = "/var/cache/fw-fanctrl-rs";

/// Reads a .curvedef file and returns a [`ParsedCurve`].
#[inline]
fn load_curve_from_path(path: &PathBuf) -> Result<ParsedCurve, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    squash_curvedef(file)
}

/// Helper to try to write a [`FanProfile`]'s LUT to disk.
fn save_lut_to_cache(lut: &[u8], signature: u64) -> Result<(), Box<dyn std::error::Error>> {
    let cache_dir = Path::new(FAN_CACHE_DIR);
    fs::create_dir_all(cache_dir)?;
    let file_path = cache_dir.join(format!("{signature:016x}.lut"));
    let tmp_path = file_path.with_extension("tmp");
    fs::write(&tmp_path, lut)?;
    fs::rename(tmp_path, file_path)?;
    Ok(())
}

/// Compiles a [`ParsedCurve`] into a [`FanProfile`].
/// If `CACHE` is true, the compiled curve will be cached to `/var/cache/fw-fanctrl-rs/CURVE_XXH3_64_HASH.lut`.
fn compile_curve<const CACHE: bool>(
    parsed_curve: ParsedCurve,
) -> Result<FanProfile, Box<dyn std::error::Error>> {
    let lut = generate_fan_curve_lut_dyn(&parsed_curve.points)?;

    if CACHE {
        if let Err(e) = save_lut_to_cache(&lut, parsed_curve.signature) {
            warn!(
                "Unable to write curve \"{}\" to cache due to error: {e}\nThis program will continue to work properly, but will be forced to rebuild any and all custom curves at startup.",
                parsed_curve.name
            );
        } else {
            info!("Wrote curve \"{}\" to cache.", parsed_curve.name);
        }
    }

    Ok(FanProfile {
        name: parsed_curve.name.into(),
        start: parsed_curve.points[0].0,
        end: parsed_curve.points[parsed_curve.points.len() - 1].0,
        lut: lut.into(),
        signature: parsed_curve.signature,
    })
}

/// Loads a curve from a .curvedef file and returns a [`FanProfile`].
/// If `CACHE` is true, the compiled curve will be cached to `/var/cache/fw-fanctrl-rs/CURVE_XXH3_64_HASH.lut` if it doesn't already exist.
pub(super) fn get_external_curve(path: &PathBuf) -> Result<FanProfile, Box<dyn std::error::Error>> {
    // first, parse the curve
    let parsed_curve = load_curve_from_path(path)?;
    infov!(
        "Loaded curve \"{}\" from {}",
        parsed_curve.name,
        path.display()
    );

    // next, check if a curve with a matching signature already exists in the cache
    let cache_file = Path::new(FAN_CACHE_DIR).join(format!("{:016x}.lut", parsed_curve.signature));
    if cache_file.exists() {
        let lut = fs::read(&cache_file)?;
        infov!(
            "Found cached curve for \"{}\" at {}. Using it.",
            parsed_curve.name,
            cache_file.display()
        );
        return Ok(FanProfile {
            name: parsed_curve.name.into(),
            start: parsed_curve.points[0].0,
            end: parsed_curve.points[parsed_curve.points.len() - 1].0,
            lut: lut.into(),
            signature: parsed_curve.signature,
        });
    }
    // otherwise, compile the curve and cache it
    info!(
        "No cached curve found for {}. Compiling...",
        parsed_curve.name
    );

    compile_curve::<true>(parsed_curve)
}
