//! The WGPU backend keeps its render pipelines across flushes. A flush that
//! binds only some of them, such as a lone clear, must leave the rest cached,
//! or the next flush that draws builds them again. Past its capacity the cache
//! evicts the pipelines used longest ago, but never one used in the last few
//! flushes; that boundary is tested without a GPU in src/renderer/wgpu.rs.
#![cfg(feature = "wgpu")]

use femtovg::{renderer::WGPURenderer, BlendFactor, Canvas, Color, Paint};

mod common;
use common::headless_device;
use common::pipelines::{live_pipelines_after_flush, rect, target, SIZE};

/// Distinct pipelines the drawing flush binds, the clear pipeline included. Far
/// below the renderer's capacity, so nothing can be evicted.
const STATES: usize = 100;

/// Each round flushes `STATES - 1` fills in distinct blend states, then a
/// clear-only flush. Every pipeline stays alive, so an unchanged count after
/// the first round means no later flush built one. Before the fix the
/// clear-only flush left one pipeline alive.
#[test]
fn pipelines_stay_alive_across_flushes_that_do_not_use_them() {
    let Some((device, queue)) = headless_device() else {
        return;
    };
    let target = target(&device);
    let factors = [
        BlendFactor::One,
        BlendFactor::OneMinusSrcAlpha,
        BlendFactor::Zero,
        BlendFactor::SrcColor,
        BlendFactor::OneMinusSrcColor,
        BlendFactor::DstColor,
        BlendFactor::OneMinusDstColor,
        BlendFactor::SrcAlpha,
        BlendFactor::DstAlpha,
        BlendFactor::OneMinusDstAlpha,
    ];
    // No anti-aliasing: each fill's blend state needs one pipeline, and the clear one more.
    let red = Paint::color(Color::rgb(255, 0, 0)).with_anti_alias(false);
    let mut canvas = Canvas::new(WGPURenderer::new(device.clone(), queue.clone())).expect("canvas");
    canvas.set_size(SIZE, SIZE, 1.0);
    for round in 0..2 {
        canvas.save();
        for i in 0..STATES - 1 {
            canvas.global_composite_blend_func_separate(
                factors[i % 10],
                factors[i / 10],
                BlendFactor::One,
                BlendFactor::OneMinusSrcAlpha,
            );
            canvas.fill_path(&rect(), &red);
        }
        canvas.restore();
        let drawn = live_pipelines_after_flush(&device, &queue, &mut canvas, &target);
        canvas.clear_rect(0, 0, SIZE, SIZE, Color::black());
        let cleared = live_pipelines_after_flush(&device, &queue, &mut canvas, &target);

        let expected_drawn = if round == 0 { STATES - 1 } else { STATES };
        assert_eq!(
            drawn, expected_drawn as isize,
            "round {round}: pipelines alive after the drawing flush"
        );
        assert_eq!(
            cleared, STATES as isize,
            "round {round}: pipelines alive after the clear-only flush"
        );
    }
}
