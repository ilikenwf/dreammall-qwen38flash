//! Procedural mall layout: a looping corridor of reusable sections, plus the
//! per-loop dream mutations. Pure data — no rendering, fully unit-testable.

use crate::rng::{sub_rng, Rng};
use crate::util::{hsv, wrap, clamp};

pub const SECTION_LEN: f32 = 24.0;
pub const NUM_SECTIONS: usize = 10;
pub const LOOP_LEN: f32 = SECTION_LEN * NUM_SECTIONS as f32; // 240 m
pub const HALF_W: f32 = 7.0; // corridor half-width
pub const ATRIUM_HALF_W: f32 = 11.0;
pub const WALL_T: f32 = 0.4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionKind {
    Corridor,
    Storefronts,
    Atrium,
    FoodCourt,
    EscalatorHall,
}

#[derive(Clone, Copy, Debug)]
pub struct Section {
    pub kind: SectionKind,
    pub z0: f32,
    pub z1: f32,
    pub half_w: f32,
    pub ceiling: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PropKind {
    Bench,
    Plant,
    Vending,
    Table,
    Chair,
    Trash,
    Statue,
    Luggage,
    Mannequin,
    Column,
}

#[derive(Clone, Copy, Debug)]
pub struct Prop {
    pub id: u32,
    pub kind: PropKind,
    pub x: f32,
    pub z: f32,
    pub rot: f32,
    pub color: [f32; 3],
    pub gone: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Mirror {
    pub x: f32,
    pub z: f32,
    pub w: f32,
    pub h: f32,
    pub y: f32,
    /// Unit surface normal. Side-wall mirrors are (±1, 0); a pier mirror that
    /// faces back down the corridor at you is (0, -1).
    pub nx: f32,
    pub nz: f32,
    /// 0.0 for a mirror hung flat on a wall; otherwise the depth of the stub
    /// wall it is set into, which `world` has to build.
    pub pier: f32,
}

#[derive(Clone, Debug)]
pub struct Sign {
    pub id: u32,
    pub x: f32,
    pub z: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub text: String,
    pub bg: [f32; 3],
    pub fg: [f32; 3],
    pub tilt: f32, // rotation about z (upside down = PI)
    pub poster: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub struct Storefront {
    pub id: u32,
    pub side: i8, // -1 left, +1 right
    pub z: f32,   // center z
    pub color: [f32; 3],
    pub glass: [f32; 3],
    pub name_idx: usize,
    pub variant: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct Fountain {
    pub x: f32,
    pub z: f32,
    pub hw: f32,
    pub hd: f32,
    pub color: [f32; 3],
}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x0: f32,
    pub z0: f32,
    pub x1: f32,
    pub z1: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Escalator {
    pub side: i8,
    pub z: f32, // center z
}

pub const POSTERS: &[&str] = &[
    "NOTHING IS WRONG",
    "HAVE YOU BEEN HERE BEFORE?",
    "THE FOUNTAIN REMEMBERS",
    "WALK FORWARD",
    "SALE FOREVER",
    "YOU LOOK TIRED",
    "MALL HOURS: ALWAYS",
    "NICE SHOES",
    "DON'T LOOK BACK",
    "50% OFF YESTERDAY",
    "THE MANAGEMENT SAYS HI",
    "SUNSET LOADING...",
    "PLEASE DO NOT FEED THE ARCHITECTURE",
    "RESTROOMS THIS WAY",
    "THE PENNY SEES ALL",
    "CLOSED",
];

pub const SECTION_SIGNS: &[&str] = &[
    "FOOD COURT",
    "LEVEL ∞",
    "NOTHING THIS WAY",
    "EXIT →",
    "ATRIUM OF ECHOES",
    "GENTLE ESCALATION",
    "YOU'RE DOING GREAT",
    "STILL WALKING?",
    "THE WAY OUT IS FORWARD",
    "MALL OF THE MOON",
];

const NAME_A: &[&str] = &[
    "STAR", "MOON", "GOLDEN", "VELVET", "AZURE", "NOVA", "PEARL", "SUN", "DREAM", "CORAL",
    "ROYAL", "SILVER", "ORCHID", "AMBER",
];
const NAME_B: &[&str] = &[
    "IA", "ORA", "ETTE", "LAND", " & CO.", " PLAZA", "WORKS", "GALLERY", "BOUTIQUE", "MART",
    "ATTIRE", "TOYS", "OPTICAL", "FOODS", "SUNGLASS",
];

pub fn store_name(idx: usize) -> String {
    let a = NAME_A[idx % NAME_A.len()];
    let b = NAME_B[(idx / NAME_A.len() + idx) % NAME_B.len()];
    format!("{a}{b}")
}
pub const NUM_STORE_NAMES: usize = NAME_A.len() * NAME_B.len();

#[derive(Clone, Debug)]
pub struct Layout {
    pub seed: u64,
    pub loop_k: u32,
    pub sections: Vec<Section>,
    pub props: Vec<Prop>,
    pub mirrors: Vec<Mirror>,
    pub signs: Vec<Sign>,
    pub storefronts: Vec<Storefront>,
    pub fountains: Vec<Fountain>,
    pub escalators: Vec<Escalator>,
    pub collision: Vec<Rect>,
    pub penny: [f32; 2],
    pub hotdog: [f32; 2],
    pub hotdog_y: f32,
    pub light_tint: [f32; 3],
    pub fog: [f32; 3],
}

fn section_kinds(rng: &mut Rng) -> Vec<SectionKind> {
    use SectionKind::*;
    // Fixed skeleton so the mall is *recognizable*; the middle shuffles.
    let mut mid = vec![
        Corridor,
        Storefronts,
        FoodCourt,
        Corridor,
        Storefronts,
        Atrium,
        Corridor,
        FoodCourt,
        EscalatorHall,
    ];
    for i in (1..mid.len()).rev() {
        let j = rng.below(i + 1);
        mid.swap(i, j);
    }
    // Guarantee at least one food court and one atrium in the middle.
    if !mid.contains(&FoodCourt) {
        let k = rng.below(mid.len());
        mid[k] = FoodCourt;
    }
    if !mid.contains(&Atrium) {
        let k = rng.below(mid.len());
        mid[k] = Atrium;
    }
    let mut out = vec![Atrium]; // start at a fountain atrium
    out.extend(mid);
    out
}

impl Layout {
    /// Generate the mall for (seed, loop). loop 0 = pristine dream; later loops
    /// are the same mall with cheap mutations layered on.
    pub fn generate(seed: u64, loop_k: u32) -> Layout {
        let mut rng = Rng::new(seed);
        let kinds = section_kinds(&mut rng);

        let mut l = Layout {
            seed,
            loop_k,
            sections: Vec::new(),
            props: Vec::new(),
            mirrors: Vec::new(),
            signs: Vec::new(),
            storefronts: Vec::new(),
            fountains: Vec::new(),
            escalators: Vec::new(),
            collision: Vec::new(),
            penny: [0.0, 60.0],
            hotdog: [0.0, 0.0],
            hotdog_y: 0.84,
            light_tint: [1.0, 1.0, 1.0],
            fog: [0.72, 0.68, 0.78],
        };
        let mut next_id = 1u32;

        for (i, kind) in kinds.iter().enumerate() {
            let z0 = i as f32 * SECTION_LEN;
            let (half_w, ceiling) = match kind {
                SectionKind::Atrium => (ATRIUM_HALF_W, 8.0),
                SectionKind::EscalatorHall => (HALF_W + 1.0, 10.0),
                _ => (HALF_W, 6.0),
            };
            l.sections.push(Section {
                kind: *kind,
                z0,
                z1: z0 + SECTION_LEN,
                half_w,
                ceiling,
            });
            let z1 = z0 + SECTION_LEN;
            let zc = z0 + SECTION_LEN / 2.0;

            // side walls (collision) — one rect per section side
            l.collision.push(Rect {
                x0: -half_w - WALL_T,
                z0,
                x1: -half_w,
                z1: z0 + SECTION_LEN,
            });
            l.collision.push(Rect {
                x0: half_w,
                z0,
                x1: half_w + WALL_T,
                z1: z0 + SECTION_LEN,
            });
            // corner blocks where the corridor widens/narrows
            let prev_w = l
                .sections
                .get(i.wrapping_sub(1))
                .map(|s| s.half_w)
                .unwrap_or_else(|| l.sections.last().map(|s| s.half_w).unwrap_or(HALF_W));
            if (prev_w - half_w).abs() > 0.1 {
                let (a, b) = (prev_w.min(half_w), prev_w.max(half_w));
                for s in [-1.0, 1.0] {
                    l.collision.push(Rect {
                        x0: s * b,
                        z0: z0 - 0.01,
                        x1: s * a,
                        z1: z0 + 0.01,
                    });
                }
            }

            match kind {
                SectionKind::Corridor | SectionKind::Storefronts => {
                    // storefronts: 3 slots per side
                    for side in [-1i8, 1i8] {
                        for slot in 0..3 {
                            let sz = z0 + 4.0 + slot as f32 * 8.0;
                            l.storefronts.push(Storefront { id: next_id,                                side,
                                z: sz,
                                color: hsv(rng.f32v(), rng.range(0.25, 0.6), rng.range(0.55, 0.85)),
                                glass: hsv(rng.f32v(), rng.range(0.3, 0.7), 0.75),
                                name_idx: rng.below(NUM_STORE_NAMES),
                                variant: rng.below(4) as u8,
                            });
                            next_id += 1;
                        }
                    }
                    // occasional vending machine / plant
                    if rng.chance(0.7) {
                        let side = if rng.chance(0.5) { -5.9f32 } else { 5.9 };
                        l.push_prop(PropKind::Vending, side, rng.range(z0 + 2.0, z1 - 2.0), &mut next_id, &mut rng);
                    }
                    if rng.chance(0.6) {
                        l.push_prop(PropKind::Plant, rng.range(-5.0, 5.0), rng.range(z0 + 2.0, z1 - 2.0), &mut next_id, &mut rng);
                    }
                    // poster on a wall
                    l.signs.push(Sign { id: next_id,                        x: if rng.chance(0.5) { -half_w + 0.06 } else { half_w - 0.06 },
                        z: rng.range(z0 + 3.0, z1 - 3.0),
                        y: 2.6,                        w: 1.6,
                        h: 2.2,
                        text: String::new(),
                        bg: [0.1, 0.1, 0.1],
                        fg: [1.0, 1.0, 1.0],
                        tilt: 0.0,
                        poster: Some(rng.below(POSTERS.len())),
                    });
                    next_id += 1;
                    // small wall mirror between shops
                    if rng.chance(0.5) {
                        let side = if rng.chance(0.5) { -1.0 } else { 1.0 };
                        l.wall_mirror(half_w, rng.range(z0 + 4.0, z1 - 4.0), 2.2, 3.4, 1.9, side);
                    }
                }
                SectionKind::Atrium => {
                    let fz = zc;
                    l.fountains.push(Fountain {
                        x: 0.0,
                        z: fz,
                        hw: 3.4,
                        hd: 2.4,
                        color: hsv(rng.f32v(), 0.25, 0.8),
                    });
                    // fountain rim collision (4 sides + center pillar)
                    let (hw, hd) = (3.4f32, 2.4f32);
                    l.collision.push(Rect { x0: -hw, z0: fz - hd, x1: hw, z1: fz - hd + 0.5 });
                    l.collision.push(Rect { x0: -hw, z0: fz + hd - 0.5, x1: hw, z1: fz + hd });
                    l.collision.push(Rect { x0: -hw, z0: fz - hd, x1: -hw + 0.5, z1: fz + hd });
                    l.collision.push(Rect { x0: hw - 0.5, z0: fz - hd, x1: hw, z1: fz + hd });
                    l.collision.push(Rect { x0: -0.6, z0: fz - 0.6, x1: 0.6, z1: fz + 0.6 });

                    // benches ringing the fountain
                    for (bx, bz, rot) in [
                        (-5.5f32, fz - 2.0, 1.5708f32),
                        (5.5, fz - 2.0, -1.5708),
                        (-5.5, fz + 2.0, 1.5708),
                        (5.5, fz + 2.0, -1.5708),
                    ] {
                        l.push_prop(PropKind::Bench, bx, bz, &mut next_id, &mut rng);
                        let _ = rot;
                    }
                    // plants in the corners
                    for sx in [-1.0, 1.0] {
                        for dz in [-1.0, 1.0] {
                            l.push_prop(
                                PropKind::Plant,
                                sx * (half_w - 1.2),
                                fz + dz * (SECTION_LEN / 2.0 - 1.5),
                                &mut next_id,
                                &mut rng,
                            );
                        }
                    }
                    if rng.chance(0.6) {
                        l.push_prop(PropKind::Statue, rng.range(-7.0, 7.0), z0 + 2.5, &mut next_id, &mut rng);
                    }
                    // big mirror wall on one side
                    let side = if rng.chance(0.5) { -1.0 } else { 1.0 };
                    l.wall_mirror(half_w, fz, 5.0, 4.2, 2.3, side);
                    l.signs.push(Sign { id: next_id,                        x: 0.0,
                        z: z0 + 0.4,
                        y: 4.6,                        w: 6.0,
                        h: 1.2,
                        text: SECTION_SIGNS[4 + rng.below(2)].to_string(),
                        bg: hsv(rng.f32v(), 0.5, 0.7),
                        fg: [1.0, 0.95, 0.8],
                        tilt: 0.0,
                        poster: None,
                    });
                    next_id += 1;
                }
                SectionKind::FoodCourt => {
                    // tables in two rows
                    let mut tables = Vec::new();
                    for row in 0..2 {
                        for col in 0..3 {
                            let tx = -4.0 + col as f32 * 4.0;
                            let tz = z0 + 6.0 + row as f32 * 10.0;
                            let id = l.push_prop(PropKind::Table, tx, tz, &mut next_id, &mut rng);
                            tables.push((id, tx, tz));
                            for (cx, cz) in [(tx - 1.1, tz), (tx + 1.1, tz)] {
                                l.push_prop(PropKind::Chair, cx, cz, &mut next_id, &mut rng);
                            }
                        }
                    }
                    l.push_prop(PropKind::Trash, -half_w + 1.0, zc, &mut next_id, &mut rng);
                    l.push_prop(PropKind::Trash, half_w - 1.0, zc, &mut next_id, &mut rng);
                    // the hotdog: first food court only, on a random table
                    if l.hotdog == [0.0, 0.0] {
                        let (_, tx, tz) = *rng.pick(&tables);
                        l.hotdog = [tx, tz];
                    }
                    l.signs.push(Sign { id: next_id,                        x: 0.0,
                        z: z0 + 0.4,
                        y: 4.2,                        w: 5.0,
                        h: 1.1,
                        text: "FOOD COURT".to_string(),
                        bg: hsv(rng.f32v(), 0.55, 0.75),
                        fg: [1.0, 1.0, 0.9],
                        tilt: 0.0,
                        poster: None,
                    });
                    next_id += 1;
                    if rng.chance(0.5) {
                        let side = if rng.chance(0.5) { -1.0 } else { 1.0 };
                        l.wall_mirror(half_w, rng.range(z0 + 3.0, z1 - 3.0), 2.2, 3.4, 1.9, side);
                    }
                }
                SectionKind::EscalatorHall => {
                    l.escalators.push(Escalator { side: -1, z: zc - 5.0 });
                    l.escalators.push(Escalator { side: 1, z: zc + 5.0 });
                    // escalator collision (along walls)
                    l.collision.push(Rect { x0: -half_w, z0: zc - 13.0, x1: -half_w + 2.2, z1: zc + 3.0 });
                    l.collision.push(Rect { x0: half_w - 2.2, z0: zc - 3.0, x1: half_w, z1: zc + 13.0 });
                    l.push_prop(PropKind::Column, -3.0, zc, &mut next_id, &mut rng);
                    l.push_prop(PropKind::Column, 3.0, zc, &mut next_id, &mut rng);
                    l.signs.push(Sign { id: next_id,                        x: 0.0,
                        z: z0 + 0.4,
                        y: 5.2,                        w: 5.5,
                        h: 1.2,
                        text: SECTION_SIGNS[5 + rng.below(2)].to_string(),
                        bg: hsv(rng.f32v(), 0.5, 0.7),
                        fg: [0.9, 1.0, 1.0],
                        tilt: 0.0,
                        poster: None,
                    });
                    next_id += 1;
                    if rng.chance(0.7) {
                        l.push_prop(PropKind::Vending, half_w - 1.1, z0 + 2.5, &mut next_id, &mut rng);
                    }
                }
            }

            // The mirrors above are all on side walls, which means you only
            // ever glimpse them out of the corner of an eye and can never look
            // back. These are set into stub walls that jut from a side wall and
            // face straight back down the corridor, so they find you instead.
            // Placed between shop bays so they never block a storefront.
            if rng.chance(0.6) {
                let side = if rng.chance(0.5) { -1.0 } else { 1.0 };
                let bay = if rng.chance(0.5) { (7.3, 8.3) } else { (15.3, 16.3) };
                l.pier_mirror(half_w, z0 + rng.range(bay.0, bay.1), side);
            }
        }

        // the penny: on the floor somewhere in the first few sections ahead
        l.penny = [
            rng.range(-4.5, 4.5),
            SECTION_LEN * 1.5 + rng.range(2.0, SECTION_LEN * 2.0),
        ];

        // dream drift: light tint + fog hue wander a little each loop
        let mut mr = sub_rng(seed, loop_k, 0xDEAD);
        let hue = mr.f32v();
        l.light_tint = hsv(0.08 + hue * 0.06, 0.10 + loop_k.min(6) as f32 * 0.012, 1.0);
        l.fog = crate::util::mixc(
            [0.72, 0.68, 0.78],
            hsv(hue, 0.28, 0.78),
            clamp(loop_k as f32 * 0.12, 0.0, 0.65),
        );

        l.apply_mutations();
        l.rebuild_collision();
        l
    }

    fn push_prop(&mut self, kind: PropKind, x: f32, z: f32, id: &mut u32, rng: &mut Rng) -> u32 {
        let color = match kind {
            PropKind::Bench => hsv(rng.f32v(), 0.35, 0.6),
            PropKind::Plant => [0.2, 0.5, 0.25],
            PropKind::Vending => hsv(rng.f32v(), 0.55, 0.7),
            PropKind::Table => hsv(rng.f32v(), 0.15, 0.8),
            PropKind::Chair => hsv(rng.f32v(), 0.4, 0.65),
            PropKind::Trash => [0.45, 0.45, 0.48],
            PropKind::Statue => hsv(rng.f32v(), 0.2, 0.75),
            PropKind::Luggage => hsv(rng.f32v(), 0.45, 0.6),
            PropKind::Mannequin => [0.85, 0.8, 0.75],
            PropKind::Column => [0.75, 0.72, 0.68],
        };
        let p = Prop {
            id: *id,
            kind,
            x,
            z,
            rot: if matches!(kind, PropKind::Chair | PropKind::Luggage) {
                rng.range(0.0, std::f32::consts::TAU)
            } else {
                0.0
            },
            color,
            gone: false,
        };
        *id += 1;
        let my_id = p.id;
        self.props.push(p);
        my_id
    }

    /// A mirror hung flat on a side wall, facing across the corridor.
    fn wall_mirror(&mut self, half_w: f32, z: f32, w: f32, h: f32, y: f32, side: f32) {
        self.mirrors.push(Mirror {
            x: side * (half_w - 0.08),
            z,
            w,
            h,
            y,
            nx: -side,
            nz: 0.0,
            pier: 0.0,
        });
    }

    /// A mirror set into a stub wall jutting from a side wall, facing back down
    /// the corridor. Side-wall mirrors whiz past and you can never turn around,
    /// so these are the ones you actually see yourself in.
    fn pier_mirror(&mut self, half_w: f32, z: f32, side: f32) {
        // The glass has to fit on the stub wall it is set into, frame included,
        // or it floats off the edge into thin air.
        let depth = 1.7;
        let w = 1.3;
        assert!(w / 2.0 + 0.15 <= depth / 2.0);
        let (x0, x1) = if side < 0.0 {
            (-half_w, -half_w + depth)
        } else {
            (half_w - depth, half_w)
        };
        self.mirrors.push(Mirror {
            x: (x0 + x1) / 2.0,
            z,
            w,
            h: 3.2,
            y: 1.75,
            nx: 0.0,
            nz: -1.0,
            pier: depth,
        });
        self.collision.push(Rect {
            x0,
            z0: z - 0.3,
            x1,
            z1: z + 0.8,
        });
    }

    /// Layer the loop-k mutations. Win-critical objects (fountains, penny,
    /// hotdog) never move or vanish — only their colors may drift.
    fn apply_mutations(&mut self) {
        if self.loop_k == 0 {
            return;
        }
        let seed = self.seed;
        let k = self.loop_k;

        for p in self.props.iter_mut() {
            let mut r = sub_rng(seed, k, p.id as u64);
            if r.chance(0.06) {
                p.gone = true;
            } else {
                if r.chance(0.18) {
                    let mut rr = sub_rng(seed, k, p.id as u64 + 7777);
                    p.color = hsv(rr.f32v(), rr.range(0.3, 0.7), rr.range(0.5, 0.85));
                }
                if r.chance(0.10) {
                    let mut rr = sub_rng(seed, k, p.id as u64 + 1234);
                    if rr.chance(0.3) {
                        // familiar object, wrong place: dropped in the middle
                        // of the walkway
                        p.x = rr.range(-2.5, 2.5);
                        p.z = wrap(p.z + rr.range(-3.0, 3.0), LOOP_LEN);
                    } else {
                        p.x = clamp(p.x + rr.range(-1.5, 1.5), -ATRIUM_HALF_W + 1.0, ATRIUM_HALF_W - 1.0);
                        p.z = wrap(p.z + rr.range(-2.0, 2.0), LOOP_LEN);
                    }
                    p.rot = rr.range(0.0, std::f32::consts::TAU);
                }
            }
        }

        for s in self.signs.iter_mut() {
            let mut r = sub_rng(seed, k, s.id as u64 + 31337);
            if r.chance(0.08) {
                s.tilt = std::f32::consts::PI; // upside down
            } else if r.chance(0.06) {
                s.tilt = r.range(-0.5, 0.5);
            }
            if r.chance(0.12) {
                s.bg = hsv(r.f32v(), r.range(0.4, 0.8), r.range(0.5, 0.8));
            }
            if r.chance(0.10) {
                s.text = SECTION_SIGNS[r.below(SECTION_SIGNS.len())].to_string();
            }
            if let Some(pp) = s.poster.as_mut() {
                let mut r2 = sub_rng(seed, k, s.id as u64 + 4242);
                if r2.chance(0.15) {
                    *pp = r2.below(POSTERS.len());
                }
            }
        }

        for sf in self.storefronts.iter_mut() {
            let mut r = sub_rng(seed, k, sf.id as u64 + 5150);
            if r.chance(0.10) {
                sf.color = hsv(r.f32v(), r.range(0.25, 0.6), r.range(0.55, 0.85));
            }
            if r.chance(0.08) {
                sf.name_idx = r.below(NUM_STORE_NAMES);
            }
            if r.chance(0.08) {
                sf.variant = r.below(4) as u8;
            }
        }

        for f in self.fountains.iter_mut() {
            let mut r = sub_rng(seed, k, (f.z * 10.0) as u64 + 9090);
            if r.chance(0.25) {
                f.color = hsv(r.f32v(), r.range(0.2, 0.5), r.range(0.6, 0.9));
            }
        }

        // familiar objects appearing where they shouldn't be
        let mut r = sub_rng(seed, k, 0xBEEF);
        for s in 0..NUM_SECTIONS {
            if r.chance(0.12) {
                let kinds = [PropKind::Bench, PropKind::Vending, PropKind::Plant, PropKind::Luggage, PropKind::Mannequin];
                let kind = *r.pick(&kinds);
                let z = s as f32 * SECTION_LEN + r.range(2.0, SECTION_LEN - 2.0);
                let x = r.range(-2.0, 2.0); // dead center of the corridor
                let id = self.props.iter().map(|p| p.id).max().unwrap_or(0) + 1;
                let mut rr = Rng::new(seed ^ (id as u64) ^ (k as u64));
                let color = hsv(rr.f32v(), 0.4, 0.7);
                self.props.push(Prop {
                    id,
                    kind,
                    x,
                    z,
                    rot: rr.range(0.0, std::f32::consts::TAU),
                    color,
                    gone: false,
                });
            }
        }
    }

    /// Static collision from props that block walking (post-mutation).
    fn rebuild_collision(&mut self) {
        for p in &self.props {
            if p.gone {
                continue;
            }
            let (hw, hd) = match p.kind {
                PropKind::Vending => (0.6, 0.45),
                PropKind::Table => (0.55, 0.55),
                PropKind::Statue => (0.7, 0.7),
                PropKind::Column => (0.45, 0.45),
                PropKind::Bench => (0.5, 1.1),
                PropKind::Trash => (0.35, 0.35),
                PropKind::Plant => (0.45, 0.45),
                _ => continue,
            };
            self.collision.push(Rect {
                x0: p.x - hw,
                z0: p.z - hd,
                x1: p.x + hw,
                z1: p.z + hd,
            });
        }
        // storefront recesses are solid (glass fronts at the wall line are
        // already covered by the side walls)
    }

    /// Which fountain (if any) contains this ground point.
    pub fn fountain_at(&self, x: f32, z: f32) -> Option<usize> {
        let z = wrap(z, LOOP_LEN);
        self.fountains.iter().position(|f| {
            (x - f.x).abs() < f.hw - 0.15 && (wrap(z - f.z + LOOP_LEN / 2.0, LOOP_LEN) - LOOP_LEN / 2.0).abs() < f.hd - 0.15
        })
    }

    pub fn section_at(&self, z: f32) -> &Section {
        let z = wrap(z, LOOP_LEN);
        let i = ((z / SECTION_LEN) as usize).min(NUM_SECTIONS - 1);
        &self.sections[i]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_deterministic() {
        let a = Layout::generate(1234, 0);
        let b = Layout::generate(1234, 0);
        assert_eq!(a.props.len(), b.props.len());
        for (x, y) in a.props.iter().zip(b.props.iter()) {
            assert_eq!(x.x.to_bits(), y.x.to_bits());
            assert_eq!(x.z.to_bits(), y.z.to_bits());
        }
    }

    #[test]
    fn guarantees() {
        for seed in [1u64, 99, 31337, 424242] {
            let l = Layout::generate(seed, 0);
            assert_eq!(l.sections.len(), NUM_SECTIONS);
            assert!(l.fountains.len() >= 2, "need fountains for the win");
            assert!(l
                .fountains
                .iter()
                .all(|f| f.z >= 0.0 && f.z < LOOP_LEN && f.x.abs() < ATRIUM_HALF_W));
            assert!(l.penny[1] > 0.0 && l.penny[1] < LOOP_LEN);
            assert!(l.hotdog != [0.0, 0.0]);
            assert!(!l.mirrors.is_empty());
            assert!(!l.storefronts.is_empty());
            assert!(!l.signs.is_empty());
            assert!(l.props.iter().any(|p| p.kind == PropKind::Table));
        }
    }

    #[test]
    fn some_mirrors_face_you_down_the_corridor() {
        // Side-wall mirrors were all there ever was, and since you can never
        // turn around you never actually saw yourself in one.
        for seed in [1u64, 42, 4242, 99999] {
            let l = Layout::generate(seed, 0);
            let facing: Vec<&Mirror> = l.mirrors.iter().filter(|m| m.nz < -0.5).collect();
            assert!(
                !facing.is_empty(),
                "seed {seed}: no mirror faces the player as they walk forward"
            );
            for m in facing {
                // It has to be standing on a stub wall, and fit inside the
                // corridor it claims to be in.
                assert!(m.pier > 0.0);
                let hw = l.section_at(m.z).half_w;
                // The pier is centred on x and reaches half its depth each way,
                // so its outer face must land on (not through) the wall.
                assert!(m.x.abs() + m.pier / 2.0 <= hw + 1e-3, "pier pokes through the wall");
                assert!(m.x.abs() - m.pier / 2.0 > 0.0, "pier crosses the corridor centre");
                assert!(m.w / 2.0 + 0.15 <= m.pier / 2.0 + 1e-3, "glass overhangs the pier");
                assert!(m.z >= 0.0 && m.z < LOOP_LEN);
            }
            // Every normal stays unit and axis-aligned, or the reflection math
            // in main.rs (a cheap axis flip) silently lies.
            for m in &l.mirrors {
                let len = (m.nx * m.nx + m.nz * m.nz).sqrt();
                assert!((len - 1.0).abs() < 1e-6, "non-unit normal {:?}", (m.nx, m.nz));
                assert!(m.nx == 0.0 || m.nz == 0.0, "mirror normal must be axis-aligned");
            }
        }
    }

    #[test]
    fn pier_mirrors_never_block_the_exits() {
        // Stub walls are obstacles; they must never sit on the penny, the
        // hotdog, or a fountain, or the mall becomes unwinnable.
        for seed in [3u64, 77, 2024] {
            let l = Layout::generate(seed, 0);
            for m in l.mirrors.iter().filter(|m| m.pier > 0.0) {
                let near = |x: f32, z: f32| (m.x - x).abs() < m.pier + 1.0 && (m.z - z).abs() < 2.0;
                assert!(!near(l.penny[0], l.penny[1]), "pier sits on the penny");
                assert!(!near(l.hotdog[0], l.hotdog[1]), "pier sits on the hotdog");
                for f in &l.fountains {
                    assert!(!near(f.x, f.z), "pier sits on a fountain");
                }
            }
        }
    }

    #[test]
    fn mutations_are_deterministic_and_preserve_goals() {
        for seed in [7u64, 555] {
            let a = Layout::generate(seed, 3);
            let b = Layout::generate(seed, 3);
            assert_eq!(a.props.len(), b.props.len());
            assert_eq!(a.penny, b.penny);
            assert_eq!(a.hotdog, b.hotdog);
            // fountains never move or vanish
            let base = Layout::generate(seed, 0);
            assert_eq!(base.fountains.len(), a.fountains.len());
            for (f0, f1) in base.fountains.iter().zip(a.fountains.iter()) {
                assert_eq!(f0.x, f1.x);
                assert_eq!(f0.z, f1.z);
                assert_eq!(f0.hw, f1.hw);
            }
            // and the dream actually drifts: some prop differs
            let mut diff = 0;
            for (p0, p1) in base.props.iter().zip(
                a.props.iter().filter(|p| base.props.iter().any(|q| q.id == p.id)),
            ) {
                if p0.gone || (p0.x - p1.x).abs() > 1e-4 || (p0.color[0] - p1.color[0]).abs() > 1e-4 {
                    diff += 1;
                }
            }
            assert!(diff > 0, "loop 3 should differ from loop 0");
        }
    }

    #[test]
    fn fountain_at_hits_basin() {
        let l = Layout::generate(42, 0);
        let f = &l.fountains[0];
        assert_eq!(l.fountain_at(f.x, f.z), Some(0));
        assert_eq!(l.fountain_at(f.x + f.hw + 1.0, f.z), None);
    }

    #[test]
    fn penny_reachable_in_corridor() {
        for seed in [3u64, 88, 1001] {
            let l = Layout::generate(seed, 0);
            let s = l.section_at(l.penny[1]);
            assert!(
                l.penny[0].abs() < s.half_w - 1.0,
                "penny must sit inside the walkway"
            );
        }
    }
}
