//! How much the app draws (step 8.4b): presets for weaker computers.
//!
//! Only what's drawn changes; the physics is the same at every preset. Low
//! draws the sky and the globes at half the screen's resolution, a quarter
//! of the belts' asteroids, and the planets' maps at half their size;
//! Medium in between; High everything at full size. Auto picks one from the
//! GPU and the processor (see [`Quality::auto`]).

use eframe::egui_wgpu::wgpu::DeviceType;

/// A preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Low,
    Medium,
    High,
}

impl Quality {
    /// Every preset, lowest first.
    pub const ALL: [Quality; 3] = [Quality::Low, Quality::Medium, Quality::High];

    /// Its name, as the interface writes it.
    pub fn name(self) -> &'static str {
        match self {
            Quality::Low => "Low",
            Quality::Medium => "Medium",
            Quality::High => "High",
        }
    }

    /// From its name, as given on the command line.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|q| q.name().eq_ignore_ascii_case(name))
    }

    /// What it changes, in words.
    pub fn describe(self) -> &'static str {
        match self {
            Quality::Low => {
                "The sky and globes at half the screen's resolution, a quarter of the belts' \
                 asteroids, the planets' maps at 1K: for weaker computers"
            }
            Quality::Medium => {
                "The sky and globes at three quarters of the screen's resolution, half the belts' \
                 asteroids"
            }
            Quality::High => "Everything at full size",
        }
    }

    /// The sky's and the globes' resolution, as a fraction of the
    /// screen's.
    pub fn render_scale(self) -> f32 {
        match self {
            Quality::Low => 0.5,
            Quality::Medium => 0.75,
            Quality::High => 1.0,
        }
    }

    /// The resolution of rays traced near a black hole, as a fraction of
    /// the screen's: each costs a hundred steps or more, so even High
    /// traces at half (see `draw_sky`).
    pub fn traced_scale(self) -> f32 {
        match self {
            Quality::Low => 0.25,
            Quality::Medium => 0.35,
            Quality::High => 0.5,
        }
    }

    /// The galaxy's glow is drawn at one over this of the view's
    /// resolution, at most.
    pub fn glow_divisor(self) -> f64 {
        match self {
            Quality::Low => 5.0,
            Quality::Medium => 4.0,
            Quality::High => 3.0,
        }
    }

    /// One in this many of the belts' asteroids drawn (and carried to now
    /// for drawing). All of them move in the physics whatever this is.
    pub fn belt_stride(self) -> u32 {
        match self {
            Quality::Low => 4,
            Quality::Medium => 2,
            Quality::High => 1,
        }
    }

    /// The widest a surface map is uploaded to the GPU, pixels: wider ones
    /// are halved until they fit.
    pub fn map_width(self) -> u32 {
        match self {
            Quality::Low => 1024,
            Quality::Medium | Quality::High => 2048,
        }
    }

    /// The preset for this computer: High for a discrete GPU, or an
    /// integrated one of the newer kinds with 8 or more processor threads;
    /// Medium for other integrated GPUs (Intel's HD and UHD Graphics, and
    /// any with fewer threads); Low for software rendering, a virtual GPU,
    /// or 4 threads or fewer with anything but a discrete GPU.
    pub fn auto(gpu: &str, kind: DeviceType, threads: usize) -> Self {
        let name = gpu.to_lowercase();
        let older_intel =
            name.contains("intel") && (name.contains("uhd") || name.contains("hd graphics"));
        match kind {
            DeviceType::DiscreteGpu => Quality::High,
            DeviceType::Cpu | DeviceType::VirtualGpu => Quality::Low,
            _ if threads <= 4 => Quality::Low,
            DeviceType::IntegratedGpu if !older_intel && threads >= 8 => Quality::High,
            _ => Quality::Medium,
        }
    }
}

/// Halves an RGBA image's width and height, each pixel the mean of the four
/// it covers, until it's no wider than `width`.
pub fn fit_map(mut image: (u32, u32, Vec<u8>), width: u32) -> (u32, u32, Vec<u8>) {
    while image.0 > width && image.0 >= 2 && image.1 >= 2 {
        let (w, h, rgba) = &image;
        let (half_w, half_h) = (w / 2, h / 2);
        let mut out = Vec::with_capacity((half_w * half_h * 4) as usize);
        for y in 0..half_h {
            for x in 0..half_w {
                for c in 0..4 {
                    let at = |dx: u32, dy: u32| {
                        u32::from(rgba[(((2 * y + dy) * w + 2 * x + dx) * 4 + c) as usize])
                    };
                    let sum = at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1);
                    out.push(((sum + 2) / 4) as u8);
                }
            }
        }
        image = (half_w, half_h, out);
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_picks_by_the_gpu_and_the_processor() {
        let iris = ("Intel(R) Iris(R) Xe Graphics", DeviceType::IntegratedGpu);
        let uhd = ("Intel(R) UHD Graphics 620", DeviceType::IntegratedGpu);
        let geforce = ("NVIDIA GeForce RTX 3060", DeviceType::DiscreteGpu);
        let software = ("Microsoft Basic Render Driver", DeviceType::Cpu);
        let auto = |(name, kind): (&str, DeviceType), threads| Quality::auto(name, kind, threads);
        assert_eq!(auto(iris, 12), Quality::High);
        assert_eq!(auto(iris, 6), Quality::Medium);
        assert_eq!(auto(uhd, 8), Quality::Medium);
        assert_eq!(auto(uhd, 4), Quality::Low);
        assert_eq!(auto(geforce, 4), Quality::High);
        assert_eq!(auto(software, 16), Quality::Low);
    }

    #[test]
    fn presets_draw_less_from_high_to_low() {
        for pair in Quality::ALL.windows(2) {
            let (lower, higher) = (pair[0], pair[1]);
            assert!(lower.render_scale() < higher.render_scale());
            assert!(lower.traced_scale() < higher.traced_scale());
            assert!(lower.glow_divisor() > higher.glow_divisor());
            assert!(lower.belt_stride() > higher.belt_stride());
            assert!(lower.map_width() <= higher.map_width());
        }
        assert_eq!(Quality::parse("low"), Some(Quality::Low));
        assert_eq!(Quality::parse("HIGH"), Some(Quality::High));
        assert_eq!(Quality::parse("ultra"), None);
    }

    #[test]
    fn a_map_is_halved_to_fit_each_pixel_the_mean_of_four() {
        // 4 × 2, two flat halves, to 2 × 1: each half's color, kept.
        let mut rgba = Vec::new();
        for _ in 0..2 {
            rgba.extend([10, 20, 30, 255].repeat(2));
            rgba.extend([200, 100, 50, 255].repeat(2));
        }
        let (w, h, out) = fit_map((4, 2, rgba), 2);
        assert_eq!((w, h), (2, 1));
        assert_eq!(out, [10, 20, 30, 255, 200, 100, 50, 255]);
        // A 2048-wide map fits a 1024 limit in one halving; a narrow one is
        // left alone.
        let big = (2048, 1024, vec![0; 2048 * 1024 * 4]);
        assert_eq!(fit_map(big, 1024).0, 1024);
        assert_eq!(fit_map((512, 256, vec![0; 512 * 256 * 4]), 1024).0, 512);
    }
}
