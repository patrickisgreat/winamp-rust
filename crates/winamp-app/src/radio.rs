//! Built-in internet radio stations (Shoutcast / Icecast streams).

pub struct Station {
    pub name: &'static str,
    pub url: &'static str,
}

pub struct Group {
    pub name: &'static str,
    pub stations: &'static [Station],
}

const fn s(name: &'static str, url: &'static str) -> Station {
    Station { name, url }
}

pub const GROUPS: &[Group] = &[
    Group {
        name: "SomaFM",
        stations: &[
            s(
                "Groove Salad (ambient beats)",
                "https://ice1.somafm.com/groovesalad-128-mp3",
            ),
            s(
                "Drone Zone (ambient space)",
                "https://ice1.somafm.com/dronezone-128-mp3",
            ),
            s(
                "Space Station Soma",
                "https://ice1.somafm.com/spacestation-128-mp3",
            ),
            s(
                "Lush (vocal electronica)",
                "https://ice1.somafm.com/lush-128-mp3",
            ),
            s(
                "Secret Agent (spy jazz)",
                "https://ice1.somafm.com/secretagent-128-mp3",
            ),
            s("DEF CON Radio", "https://ice1.somafm.com/defcon-128-mp3"),
            s(
                "Beat Blender",
                "https://ice1.somafm.com/beatblender-128-mp3",
            ),
            s("Underground 80s", "https://ice1.somafm.com/u80s-128-mp3"),
            s(
                "Indie Pop Rocks!",
                "https://ice1.somafm.com/indiepop-128-mp3",
            ),
            s(
                "Boot Liquor (americana)",
                "https://ice1.somafm.com/bootliquor-128-mp3",
            ),
            s(
                "Deep Space One",
                "https://ice1.somafm.com/deepspaceone-128-mp3",
            ),
            s(
                "Fluid (instrumental hip-hop)",
                "https://ice1.somafm.com/fluid-128-mp3",
            ),
            s("Metal Detector", "https://ice1.somafm.com/metal-128-mp3"),
            s("Seven Inch Soul", "https://ice1.somafm.com/7soul-128-mp3"),
        ],
    },
    Group {
        name: "Radio Paradise",
        stations: &[
            s(
                "Main Mix (MP3 192k)",
                "https://stream.radioparadise.com/mp3-192",
            ),
            s("Mellow Mix", "https://stream.radioparadise.com/mellow-192"),
            s("Rock Mix", "https://stream.radioparadise.com/rock-192"),
            s("Global Mix", "https://stream.radioparadise.com/global-192"),
        ],
    },
    Group {
        name: "Electronic",
        stations: &[
            s(
                "Nightride FM (synthwave)",
                "https://stream.nightride.fm/nightride.mp3",
            ),
            s("Chillsky", "https://lfhh.radioca.st/stream"),
            s(
                "Ambient Sleeping Pill",
                "https://radio.stereoscenic.com/asp-h",
            ),
        ],
    },
    Group {
        name: "Jazz & Classical",
        stations: &[
            s(
                "Jazz24",
                "https://live.amperwave.net/direct/ppm-jazz24mp3-ibc1",
            ),
            s(
                "WFMT Chicago (classical)",
                "https://wfmt.streamguys1.com/main-mp3",
            ),
            s("Linn Jazz", "http://radio.linn.co.uk:8003/autodj"),
        ],
    },
];
