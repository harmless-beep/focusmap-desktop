//! What the city is made of.
//!
//! The app hands over one snapshot of today's usage; this module turns it into
//! a deterministic list of buildings. Same snapshot in, same city out, so the
//! view never flickers between frames and a replay can rebuild any moment.
//!
//! App towers are the interactive part. Around them sit procedural filler
//! blocks, gap clusters and a distant skyline ring; all use the same instanced
//! box path, but only named app towers can be hovered or pinned.

use std::collections::BTreeMap;

/// The five districts, in ring order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum District {
    Coding,
    Communication,
    Browsing,
    Entertainment,
    Creative,
}

impl District {
    pub const ALL: [District; 5] = [
        District::Coding,
        District::Communication,
        District::Browsing,
        District::Entertainment,
        District::Creative,
    ];

    /// Ring angle in degrees, measured from +Z toward +X.
    fn angle(self) -> f32 {
        match self {
            District::Entertainment => 0.0,
            District::Creative => 72.0,
            District::Communication => 144.0,
            District::Browsing => 216.0,
            District::Coding => 288.0,
        }
    }

    /// Neon accent, in display-referred sRGB.
    pub fn neon(self) -> [f32; 3] {
        match self {
            District::Coding => [0.10, 0.95, 1.00],
            District::Communication => [1.00, 0.18, 0.72],
            District::Browsing => [0.55, 1.00, 0.35],
            District::Entertainment => [1.00, 0.66, 0.18],
            District::Creative => [0.62, 0.36, 1.00],
        }
    }

    /// Daytime facade paint: strong pastel, so the city reads as a toy
    /// diorama in sunlight and keeps its district identity at a glance.
    fn facade(self) -> [f32; 3] {
        match self {
            District::Coding => [0.30, 0.72, 0.74],
            District::Communication => [0.90, 0.50, 0.66],
            District::Browsing => [0.62, 0.78, 0.42],
            District::Entertainment => [0.94, 0.66, 0.34],
            District::Creative => [0.58, 0.50, 0.86],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            District::Coding => "Coding",
            District::Communication => "Communication",
            District::Browsing => "Browsing",
            District::Entertainment => "Entertainment",
            District::Creative => "Creative",
        }
    }
}

/// Neutral filler walls mixed into every district so the palette does not
/// read as five solid stripes.
const FILLER_PAINT: [[f32; 3]; 6] = [
    [0.86, 0.83, 0.73],
    [0.72, 0.82, 0.74],
    [0.80, 0.72, 0.84],
    [0.92, 0.78, 0.66],
    [0.68, 0.78, 0.84],
    [0.89, 0.87, 0.91],
];

/// Today's mood, derived from the live score.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Deep,
    Steady,
    Distracted,
    Idle,
}

impl Mood {
    pub fn label(self) -> &'static str {
        match self {
            Mood::Deep => "Deep focus",
            Mood::Steady => "Steady",
            Mood::Distracted => "Scattered",
            Mood::Idle => "Standby",
        }
    }

    /// Shader-side code: 0 deep, 1 steady, 2 distracted, 3 idle.
    pub(crate) fn code(self) -> f32 {
        match self {
            Mood::Deep => 0.0,
            Mood::Steady => 1.0,
            Mood::Distracted => 2.0,
            Mood::Idle => 3.0,
        }
    }
}

/// One app as the city sees it.
#[derive(Clone, Debug)]
pub struct CityApp {
    pub name: String,
    pub district: District,
    pub minutes: u32,
    pub score: f32,
    pub switches: u32,
    /// Foreground app right now.
    pub active: bool,
}

#[derive(Clone, Debug, Default)]
pub struct CitySnapshot {
    pub apps: Vec<CityApp>,
    pub mood: Mood,
    pub score: f32,
}

impl Default for Mood {
    fn default() -> Self {
        Mood::Idle
    }
}

#[derive(Clone, Debug)]
pub struct Building {
    /// Empty for scenery; only named buildings can be hovered or pinned.
    pub app: String,
    pub district: District,
    pub center: [f32; 3],
    pub half: [f32; 3],
    pub body: [f32; 3],
    pub glow: [f32; 3],
    pub seed: f32,
    /// Shown in the hover card, so the view needs no second data source.
    pub minutes: u32,
    pub score: f32,
    pub switches: u32,
    pub active: bool,
}

pub const MAX_BUILDINGS: usize = 192;
const MAX_APPS: usize = 48;
const SLOTS: usize = 5;
const ROWS: usize = 5;
const SLOT_GAP: f32 = 3.4;
const DISTRICT_RADIUS: f32 = 26.0;
const CORE_HALF: f32 = 3.0;
const BACKGROUND_RING: usize = 32;
const GAP_CLUSTERS: usize = 5;

/// Build the city. Apps are sorted by minutes so the tall ones keep their
/// plot as usage shifts; the core tower always stands in the middle.
pub fn layout(snapshot: &CitySnapshot) -> Vec<Building> {
    let mut apps: Vec<&CityApp> = snapshot.apps.iter().collect();
    apps.sort_by(|a, b| {
        b.minutes
            .cmp(&a.minutes)
            .then_with(|| a.name.cmp(&b.name))
    });
    apps.truncate(MAX_APPS);

    let max_minutes = apps.iter().map(|a| a.minutes).max().unwrap_or(1).max(1) as f32;

    let mut by_district: BTreeMap<District, Vec<&CityApp>> = BTreeMap::new();
    for app in &apps {
        by_district.entry(app.district).or_default().push(app);
    }

    let mut out = Vec::with_capacity(MAX_BUILDINGS);
    let mut tallest: f32 = 4.0;

    for (district, list) in &by_district {
        let angle = district.angle().to_radians();
        let (sin, cos) = angle.sin_cos();
        let origin = [sin * DISTRICT_RADIUS, 0.0, cos * DISTRICT_RADIUS];
        let neon = district.neon();
        let facade = district.facade();
        let plots = SLOTS * ROWS;

        // Apps take the first plots; the rest become short neighbourhood
        // blocks so a quiet day still looks like a city, not a plinth.
        let app_count = list.len().min(plots);
        for (slot, app) in list.iter().take(app_count).enumerate() {
            let (col, row) = grid_offset(slot);
            let h = hash(&app.name, 1);
            let h2 = hash(&app.name, 2);
            let h3 = hash(&app.name, 3);

            let center = [
                origin[0] + col * SLOT_GAP + (h - 0.5) * 0.35,
                0.0,
                origin[2] + row * SLOT_GAP + (h2 - 0.5) * 0.35,
            ];

            let ratio = (app.minutes as f32 / max_minutes).clamp(0.0, 1.0);
            let mut height = 1.8 + 7.2 * ratio.powf(0.62) + h3 * 1.3;
            if app.active {
                height *= 1.15;
            }
            let width = 1.25 + h * 0.50;
            let depth = 1.25 + h2 * 0.50;

            let mut glow = neon;
            let tint = 0.82 + h3 * 0.18;
            for c in glow.iter_mut() {
                *c *= tint;
            }
            // One in five blocks is a painted cream shop-house; the rest wear
            // the district colour, with a little variation per app.
            let body = if h < 0.20 {
                FILLER_PAINT[0]
            } else {
                scale(facade, 0.88 + h2 * 0.22)
            };

            tallest = tallest.max(height + 2.0);
            out.push(Building {
                app: app.name.clone(),
                district: *district,
                center: [center[0], height * 0.5, center[2]],
                half: [width, height * 0.5, depth],
                body,
                glow,
                seed: h3,
                minutes: app.minutes,
                score: app.score,
                switches: app.switches,
                active: app.active,
            });
        }

        for slot in app_count..plots {
            let salt = format!("{:?}-{slot}", district);
            let h = hash(&salt, 5);
            if h < 0.14 {
                continue; // a service alley keeps the blocks from merging
            }
            let h2 = hash(&salt, 6);
            let h3 = hash(&salt, 7);
            let (col, row) = grid_offset(slot);
            let height = 1.1 + 3.8 * h3.powf(1.4);
            let width = 1.55 + h * 0.50;
            let depth = 1.55 + h2 * 0.50;
            out.push(Building {
                app: String::new(),
                district: *district,
                center: [
                    origin[0] + col * SLOT_GAP + (h - 0.5) * 0.30,
                    height * 0.5,
                    origin[2] + row * SLOT_GAP + (h2 - 0.5) * 0.30,
                ],
                half: [width, height * 0.5, depth],
                body: filler_paint(&salt, *district),
                glow: scale(neon, 0.55),
                seed: h3,
                minutes: 0,
                score: 0.0,
                switches: 0,
                active: false,
            });
        }
    }

    // Close the wedges between the five district blocks so the ring reads as
    // one continuous city.
    for gap in 0..GAP_CLUSTERS {
        let angle = (36.0 + gap as f32 * 72.0).to_radians();
        let (sin, cos) = angle.sin_cos();
        for cell in 0..4 {
            let salt = format!("gap-{gap}-{cell}");
            let h = hash(&salt, 11);
            let h2 = hash(&salt, 12);
            let h3 = hash(&salt, 13);
            let height = 1.4 + 3.6 * h3;
            let dx = if cell % 2 == 0 { -1.7 } else { 1.7 };
            let dz = if cell < 2 { -1.7 } else { 1.7 };
            let r = 27.5 + dz + h2 * 2.0;
            out.push(Building {
                app: String::new(),
                district: District::ALL[(gap + 2) % 5],
                center: [sin * r + cos * dx, height * 0.5, cos * r - sin * dx],
                half: [1.7 + h * 0.5, height * 0.5, 1.7 + h2 * 0.5],
                body: filler_paint(&salt, District::ALL[(gap + 2) % 5]),
                glow: scale(District::ALL[(gap + 2) % 5].neon(), 0.40),
                seed: h3,
                minutes: 0,
                score: 0.0,
                switches: 0,
                active: false,
            });
        }
    }

    // A fading downtown ring behind the interactive city, so the diorama has
    // a skyline instead of ending on bare ground.
    for i in 0..BACKGROUND_RING {
        let salt = format!("far-{i}");
        let h = hash(&salt, 21);
        let h2 = hash(&salt, 22);
        let h3 = hash(&salt, 23);
        let angle = i as f32 / BACKGROUND_RING as f32 * std::f32::consts::TAU + h * 0.08;
        let radius = 41.0 + h2 * 10.0;
        let height = 2.0 + 6.5 * h3;
        let width = 2.6 + h * 2.0;
        out.push(Building {
            app: String::new(),
            district: District::ALL[i % 5],
            center: [
                angle.cos() * radius,
                height * 0.5,
                angle.sin() * radius,
            ],
            half: [width, height * 0.5, width * (0.8 + h2 * 0.5)],
            body: scale(FILLER_PAINT[(i + 1) % FILLER_PAINT.len()], 0.86),
            glow: scale(District::ALL[i % 5].neon(), 0.25),
            seed: h3,
            minutes: 0,
            score: 0.0,
            switches: 0,
            active: false,
        });
    }

    // The core tower stands over the plaza, shorter than the tallest app so
    // it never hides the whole far side of the city from the high camera.
    let core_height = (tallest + 1.0).clamp(9.5, 13.0);
    let minutes = apps.iter().map(|a| a.minutes).sum();
    let switches = apps.iter().map(|a| a.switches).sum();
    out.push(Building {
        app: "FocusMap".to_string(),
        district: District::Coding,
        center: [0.0, core_height * 0.5, 0.0],
        half: [CORE_HALF, core_height * 0.5, CORE_HALF],
        body: [0.17, 0.22, 0.30],
        glow: [0.45, 0.95, 1.0],
        seed: 0.5,
        minutes,
        score: snapshot.score,
        switches,
        active: false,
    });

    out
}

/// Plot offset in the district block, centred on the district origin.
fn grid_offset(slot: usize) -> (f32, f32) {
    let col = (slot % SLOTS) as f32 - (SLOTS as f32 - 1.0) * 0.5;
    let row = (slot / SLOTS) as f32 - (ROWS as f32 - 1.0) * 0.5;
    (col, row)
}

fn scale(color: [f32; 3], k: f32) -> [f32; 3] {
    [color[0] * k, color[1] * k, color[2] * k]
}

fn filler_paint(salt: &str, district: District) -> [f32; 3] {
    let pick = hash(salt, 17);
    if pick < 0.22 {
        return scale(district.facade(), 0.94);
    }
    FILLER_PAINT[(pick * 10.0) as usize % FILLER_PAINT.len()]
}

/// FNV-1a over the app name and a salt, mapped to 0..1.
fn hash(name: &str, salt: u32) -> f32 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ salt as u64;
    for byte in name.as_bytes() {
        h ^= *byte as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    // Fold high bits in so short, similar names still spread out.
    h ^= h >> 29;
    (h % 1_000_003) as f32 / 1_000_003.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> CitySnapshot {
        let mut apps = Vec::new();
        for (i, district) in District::ALL.iter().cycle().take(12).enumerate() {
            apps.push(CityApp {
                name: format!("app-{i}.exe"),
                district: *district,
                minutes: 10 + i as u32 * 5,
                score: 60.0,
                switches: 2,
                active: i == 0,
            });
        }
        CitySnapshot {
            apps,
            mood: Mood::Deep,
            score: 78.0,
        }
    }

    fn named(city: &[Building]) -> Vec<&Building> {
        city.iter().filter(|b| !b.app.is_empty()).collect()
    }

    #[test]
    fn layout_is_deterministic() {
        let a = layout(&snapshot());
        let b = layout(&snapshot());
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.app, y.app);
            assert_eq!(x.center, y.center);
            assert_eq!(x.half, y.half);
        }
    }

    #[test]
    fn every_app_gets_one_building_plus_the_core() {
        let snap = snapshot();
        let city = layout(&snap);
        let apps = named(&city);
        assert_eq!(apps.len(), snap.apps.len() + 1);
        assert_eq!(city.last().unwrap().app, "FocusMap");
        // Scenery fills the empty plots, all of it un-named.
        assert!(city.len() > snap.apps.len() + 1);
        for b in city.iter().filter(|b| b.app.is_empty()) {
            assert_eq!(b.minutes, 0);
        }
    }

    #[test]
    fn buildings_stand_on_the_ground_and_clear_the_core() {
        let city = layout(&snapshot());
        let core = city.last().unwrap();
        for b in &city[..city.len() - 1] {
            assert!((b.center[1] - b.half[1]).abs() < 1e-4);
            if b.app.is_empty() {
                continue;
            }
            let dx = b.center[0] - core.center[0];
            let dz = b.center[2] - core.center[2];
            assert!(dx.hypot(dz) > CORE_HALF + b.half[0].max(b.half[2]));
        }
    }

    #[test]
    fn apps_are_capped() {
        let mut snap = snapshot();
        snap.apps = (0..80)
            .map(|i| CityApp {
                name: format!("app-{i}"),
                // Spread across districts so the app cap, not a district's
                // plots, is what gets tested.
                district: District::ALL[i as usize % District::ALL.len()],
                minutes: i,
                score: 50.0,
                switches: 0,
                active: false,
            })
            .collect();
        assert_eq!(named(&layout(&snap)).len(), MAX_APPS + 1);
    }

    #[test]
    fn the_city_fits_the_instance_capacity() {
        let mut snap = snapshot();
        snap.apps = (0..80)
            .map(|i| CityApp {
                name: format!("app-{i}"),
                district: District::ALL[i as usize % District::ALL.len()],
                minutes: i,
                score: 50.0,
                switches: 0,
                active: false,
            })
            .collect();
        assert!(layout(&snap).len() <= MAX_BUILDINGS);
    }
}
