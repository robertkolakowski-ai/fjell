//! Tabell med kjente skjermstørrelser (piksler, stående: bredde × høyde) for signal S2.
//! Landskap matches ved å bytte om bredde og høyde i `is_known_screen_size`.
//! Endring i tabellen øker `ALGO_VER` (PLAN-KJERNE 6.3).

/// (bredde, høyde) i stående retning for iPhone, iPad og stasjonær/bærbar (Retina).
pub const SCREEN_SIZES: &[(u32, u32)] = &[
    // iPhone
    (640, 1136),
    (750, 1334),
    (828, 1792),
    (1080, 1920),
    (1125, 2436),
    (1170, 2532),
    (1179, 2556),
    (1206, 2622),
    (1242, 2208),
    (1242, 2688),
    (1284, 2778),
    (1290, 2796),
    (1320, 2868),
    // iPad
    (1536, 2048),
    (1620, 2160),
    (1640, 2360),
    (1668, 2224),
    (1668, 2388),
    (2048, 2732),
    (2064, 2752),
    // Desktop: 1080p (= 1080×1920 over), 1440p, 4K og Retina (lagt som stående for felles oppslag)
    (1440, 2560),
    (2160, 3840),
    (900, 1440),
    (1600, 2560),
    (1800, 2880),
    (1964, 3024),
    (2234, 3456),
];

/// Lik en kjent skjermstørrelse i begge retninger.
pub fn is_known_screen_size(width: u32, height: u32) -> bool {
    let (short, long) = if width <= height {
        (width, height)
    } else {
        (height, width)
    };
    SCREEN_SIZES.iter().any(|&(w, h)| w == short && h == long)
}
