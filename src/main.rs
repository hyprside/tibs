use std::time::Duration;

use glow::HasContext;
use tab_app_framework::{
    Config, GlApplication, GlEventContext, GlInitContext, GlTabAppFramework, RenderEvent,
    RenderMode,
};
use tracing_subscriber::{EnvFilter, fmt};

/// hsl: [1.0, 1.0, 1.0] -> [1.0, 1.0, 1.0]
pub fn hsl_to_rgb(hsl: [f32; 3]) -> [f32; 3] {
    let hsl= [hsl[0] / 360., hsl[1], hsl[2]];
    pub const RGB_UNIT_MAX: f32 = 1.0;
    pub const RATIO_MAX: f32 = 1.0;
    pub const ALL_MIN: f32 = 0.0;

    const ONE: f32 = 1.0;
    const TWO: f32 = 2.0;
    const SIX: f32 = 6.0;

    const ONE_THIRD: f32 = ONE / 3.0;
    const TWO_THIRD: f32 = TWO / 3.0;

    pub fn bound(r: f32, entire: f32) -> f32 {
        let mut n = r;
        loop {
            let less = n < ALL_MIN;
            let bigger = n > entire;
            if !less && !bigger {
                break n;
            }
            if less {
                n += entire;
            } else {
                n -= entire;
            }
        }
    }

    pub fn bound_ratio(r: f32) -> f32 {
        bound(r, RATIO_MAX)
    }

    fn calc_rgb_unit(unit: f32, temp1: f32, temp2: f32) -> f32 {
        let mut result = temp2;
        if SIX * unit < ONE {
            result = temp2 + (temp1 - temp2) * SIX * unit
        } else if TWO * unit < ONE {
            result = temp1
        } else if 3.0 * unit < TWO {
            result = temp2 + (temp1 - temp2) * (TWO_THIRD - unit) * SIX
        }
        result * RGB_UNIT_MAX
    }

    let [h, s, l]: [f32; 3] = hsl;
    if s == 0.0 {
        let unit = RGB_UNIT_MAX * l;
        return [unit, unit, unit];
    }

    let temp1 = if l < 0.5 { l * (ONE + s) } else { l + s - l * s };

    let temp2 = TWO * l - temp1;
    let hue = h;

    let temp_r = bound_ratio(hue + ONE_THIRD);
    let temp_g = bound_ratio(hue);
    let temp_b = bound_ratio(hue - ONE_THIRD);

    let r = calc_rgb_unit(temp_r, temp1, temp2);
    let g = calc_rgb_unit(temp_g, temp1, temp2);
    let b = calc_rgb_unit(temp_b, temp1, temp2);
    [r, g, b]
}
struct Tibs {
    hsl: (f32, f32, f32),
    frames: usize
}

impl GlApplication for Tibs {
    fn init(_ctx: &mut GlInitContext) -> anyhow::Result<Self> {

        Ok(Self {hsl: (0., 1., 0.5), frames: 0})
    }

    fn on_render(&mut self, ctx: &mut GlEventContext<'_, '_, Self>, _event: RenderEvent) {
        let gl = ctx.gl().glow();
        unsafe {
            let [r,g,b] = hsl_to_rgb(self.hsl.into());
            gl.clear_color(r, g, b, 1.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }
        self.hsl.0 += 4.;
        self.hsl.0 = self.hsl.0 % 360.0;
        self.frames += 1;
        println!("{}", self.frames % 60);
    }
}

fn main() -> anyhow::Result<()> {
    
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tab_app_framework.core=debug")),
        )
        .try_init();
    let mut app = GlTabAppFramework::<Tibs>::init(|config: &mut Config| {
        config.opengl_es_version(3, 0);
        config.set_render_mode(RenderMode::Eager);
    })?;
    app.run()?;
    Ok(())
}
