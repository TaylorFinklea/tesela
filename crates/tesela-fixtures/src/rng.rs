use rand::Rng;
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// Seeded RNG used everywhere the fixtures crate needs randomness.
/// ChaCha8 is chosen for cross-platform determinism — the default
/// `rand::ThreadRng` is OS-specific, which would make fixtures
/// platform-dependent.
pub type FixtureRng = ChaCha8Rng;

pub fn rng(seed: u64) -> FixtureRng {
    ChaCha8Rng::seed_from_u64(seed)
}

/// Sample one element from a slice. Returns `&str` regardless of
/// whether the slice holds `&str`, `String`, or any other `AsRef<str>`.
/// Panics on empty.
pub fn pick<'a, S: AsRef<str> + 'a>(rng: &mut FixtureRng, slice: &'a [S]) -> &'a str {
    let idx = rng.gen_range(0..slice.len());
    slice[idx].as_ref()
}

/// Roll a uniform integer in `[lo, hi]`. Helper that's used a lot in the
/// content generators.
pub fn range(rng: &mut FixtureRng, lo: usize, hi: usize) -> usize {
    if hi <= lo {
        lo
    } else {
        rng.gen_range(lo..=hi)
    }
}

/// True with the given probability (0..=1).
pub fn chance(rng: &mut FixtureRng, p: f64) -> bool {
    rng.gen_bool(p.clamp(0.0, 1.0))
}
