use super::model::Section;

pub(super) const BASE_URL: &str = "https://www.deutschlandfunk.de";
pub(super) const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36";

/// Builtin Deutschlandfunk section landing pages. URLs verified against
/// deutschlandfunk.de. If a future redesign breaks one, verify the affected
/// landing URL in the app or with a focused parser test before updating here.
pub const SECTIONS: &[Section] = &[
    Section {
        id: "startseite",
        label: "Startseite",
        url: "https://www.deutschlandfunk.de/",
    },
    Section {
        id: "nachrichten",
        label: "Nachrichten",
        url: "https://www.deutschlandfunk.de/nachrichten-100.html",
    },
    Section {
        id: "hintergrund",
        label: "Hintergrund",
        url: "https://www.deutschlandfunk.de/hintergrund-100.html",
    },
    Section {
        id: "interview",
        label: "Interview",
        url: "https://www.deutschlandfunk.de/interview-100.html",
    },
    Section {
        id: "kommentar",
        label: "Kommentar",
        url: "https://www.deutschlandfunk.de/kommentare-und-themen-der-woche-100.html",
    },
    Section {
        id: "europa",
        label: "Europa heute",
        url: "https://www.deutschlandfunk.de/europa-heute-100.html",
    },
    Section {
        id: "wissenschaft",
        label: "Forschung aktuell",
        url: "https://www.deutschlandfunk.de/forschung-aktuell-100.html",
    },
    Section {
        id: "umwelt",
        label: "Umwelt und Verbraucher",
        url: "https://www.deutschlandfunk.de/umwelt-und-verbraucher-100.html",
    },
    Section {
        id: "kultur",
        label: "Kultur heute",
        url: "https://www.deutschlandfunk.de/kultur-heute-100.html",
    },
    Section {
        id: "buecher",
        label: "Büchermarkt",
        url: "https://www.deutschlandfunk.de/buechermarkt-100.html",
    },
    Section {
        id: "wirtschaft",
        label: "Wirtschaft am Mittag",
        url: "https://www.deutschlandfunk.de/wirtschaft-am-mittag-100.html",
    },
    Section {
        id: "sport",
        label: "Sport am Wochenende",
        url: "https://www.deutschlandfunk.de/sport-am-wochenende-100.html",
    },
    Section {
        id: "andruck",
        label: "Andruck (Sachbuch)",
        url: "https://www.deutschlandfunk.de/andruck-100.html",
    },
    Section {
        id: "essay",
        label: "Essay und Diskurs",
        url: "https://www.deutschlandfunk.de/essay-und-diskurs-100.html",
    },
    Section {
        id: "feature",
        label: "Das Feature",
        url: "https://www.deutschlandfunk.de/das-feature-100.html",
    },
    Section {
        id: "hoerspiel",
        label: "Hörspiel",
        url: "https://www.deutschlandfunk.de/hoerspiel-100.html",
    },
    Section {
        id: "presseschau",
        label: "Presseschau (DE)",
        url: "https://www.deutschlandfunk.de/presseschau-100.html",
    },
    Section {
        id: "presseschau-int",
        label: "Presseschau (intl.)",
        url: "https://www.deutschlandfunk.de/internationale-presseschau-100.html",
    },
    Section {
        id: "magazin",
        label: "Dlf-Magazin",
        url: "https://www.deutschlandfunk.de/dlf-magazin-102.html",
    },
];
