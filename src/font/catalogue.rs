/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Which font tapHLE draws when an app asks for one of the iPhone's.
//!
//! An app names a font and expects the device's copy of it. tapHLE has none of
//! them: Helvetica, Arial, Futura, Zapfino and almost every other font on an
//! iPhone is commercially licensed, and a copy being present on a device grants
//! nothing about redistributing it. So tapHLE ships open substitutes and this
//! module says which stands in for which.
//!
//! The substitutes come from a survey of the permissively licensed lookalikes
//! for Apple's font collection; `dev-scripts/fetch-fonts.py` downloads them and
//! records where each came from. What is written here is the other half: the
//! mapping, and how close each match actually is, because "close" ranges from
//! *the line breaks will be identical* to *this is a different typeface that
//! does the same job*.
//!
//! Three kinds of closeness matter, and conflating them is how a substitution
//! table becomes untrustworthy:
//!
//! - **Metric.** Designed so that text occupies the same space. Liberation Sans
//!   for Arial, Liberation Serif for Times New Roman, Liberation Mono for
//!   Courier New, Gelasio for Georgia. A layout built around the original still
//!   fits.
//! - **Shape.** Similar construction and colour, different widths. Text
//!   reflows.
//! - **Role.** Fills the same job — a script face, a chalk face — without
//!   imitating the drawing. Widths are not comparable at all.
//!
//! Helvetica is deliberately matched to Liberation Sans rather than to
//! something that looks more like it. Arial was drawn to Helvetica's metrics,
//! and Liberation Sans was drawn to Arial's, so the chain preserves the one
//! thing an emulated app cannot recover from losing: text that no longer fits
//! the button it was measured for. Someone who would rather have the closer
//! shape can say so per font in the frontend.

use std::path::PathBuf;

/// The four faces a family can be asked for. tapHLE's rasteriser has no way to
/// interpolate a variable font, so these are real files or nothing.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Style {
    Regular,
    Bold,
    Italic,
    BoldItalic,
}

impl Style {
    pub fn new(bold: bool, italic: bool) -> Style {
        match (bold, italic) {
            (false, false) => Style::Regular,
            (true, false) => Style::Bold,
            (false, true) => Style::Italic,
            (true, true) => Style::BoldItalic,
        }
    }

    pub fn is_bold(self) -> bool {
        matches!(self, Style::Bold | Style::BoldItalic)
    }

    pub fn is_italic(self) -> bool {
        matches!(self, Style::Italic | Style::BoldItalic)
    }

    /// The styles to try, in order, when a family does not have this one.
    ///
    /// Dropping the italic is less disruptive than dropping the weight: a bold
    /// label that comes out upright still reads as emphasis, while a regular
    /// one that comes out italic looks like a mistake.
    fn fallbacks(self) -> &'static [Style] {
        match self {
            Style::Regular => &[Style::Regular],
            Style::Bold => &[Style::Bold, Style::Regular],
            Style::Italic => &[Style::Italic, Style::Regular],
            Style::BoldItalic => &[
                Style::BoldItalic,
                Style::Bold,
                Style::Italic,
                Style::Regular,
            ],
        }
    }
}

/// How close a substitute is to the font it stands in for.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Closeness {
    /// Text occupies the same space, so an existing layout still fits.
    Metric,
    /// Similar construction, different widths. Text reflows.
    Shape,
    /// The same job, a different typeface. Widths are not comparable.
    Role,
}

impl Closeness {
    /// Said in the frontend, beside the name of the substitute.
    pub fn describe(self) -> &'static str {
        match self {
            Closeness::Metric => "same metrics",
            Closeness::Shape => "similar shapes",
            Closeness::Role => "same role",
        }
    }
}

/// A family tapHLE ships.
pub struct BundledFamily {
    pub id: &'static str,
    pub name: &'static str,
    pub licence: &'static str,
    /// The file for each style it has. A family with only one face lists one.
    pub faces: &'static [(Style, &'static str)],
}

impl BundledFamily {
    /// The file for a style, or for the nearest style this family has.
    pub fn file_for(&self, style: Style) -> &'static str {
        for wanted in style.fallbacks() {
            if let Some((_, file)) = self.faces.iter().find(|(have, _)| have == wanted) {
                return file;
            }
        }
        // Every family in the table below has at least a regular, and the test
        // at the bottom of this file keeps it that way.
        self.faces[0].1
    }

    pub fn has_style(&self, style: Style) -> bool {
        self.faces.iter().any(|(have, _)| *have == style)
    }
}

/// One iPhone font, and what tapHLE draws instead.
pub struct Substitution {
    /// The family name as an app sees it in `UIFont`.
    pub ios_family: &'static str,
    /// Extra names that normalise to something else — a PostScript name whose
    /// family cannot be recovered by stripping style words from it.
    pub aliases: &'static [&'static str],
    /// Which bundled family stands in.
    pub bundled: &'static str,
    pub closeness: Closeness,
    /// Said in the frontend, next to the choice.
    pub note: &'static str,
}

pub const BUNDLED: &[BundledFamily] = &[
    BundledFamily {
        id: "liberation-sans",
        name: "Liberation Sans",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "LiberationSans-Regular.ttf"),
            (Style::Bold, "LiberationSans-Bold.ttf"),
            (Style::Italic, "LiberationSans-Italic.ttf"),
            (Style::BoldItalic, "LiberationSans-BoldItalic.ttf"),
        ],
    },
    BundledFamily {
        id: "liberation-serif",
        name: "Liberation Serif",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "LiberationSerif-Regular.ttf"),
            (Style::Bold, "LiberationSerif-Bold.ttf"),
            (Style::Italic, "LiberationSerif-Italic.ttf"),
            (Style::BoldItalic, "LiberationSerif-BoldItalic.ttf"),
        ],
    },
    BundledFamily {
        id: "liberation-mono",
        name: "Liberation Mono",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "LiberationMono-Regular.ttf"),
            (Style::Bold, "LiberationMono-Bold.ttf"),
            (Style::Italic, "LiberationMono-Italic.ttf"),
            (Style::BoldItalic, "LiberationMono-BoldItalic.ttf"),
        ],
    },
    BundledFamily {
        id: "inter",
        name: "Inter",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "Inter-Regular.ttf"),
            (Style::Bold, "Inter-Bold.ttf"),
            (Style::Italic, "Inter-Italic.ttf"),
            (Style::BoldItalic, "Inter-BoldItalic.ttf"),
        ],
    },
    BundledFamily {
        id: "gelasio",
        name: "Gelasio",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "Gelasio-Regular.ttf"),
            (Style::Bold, "Gelasio-Bold.ttf"),
            (Style::Italic, "Gelasio-Italic.ttf"),
            (Style::BoldItalic, "Gelasio-BoldItalic.ttf"),
        ],
    },
    BundledFamily {
        id: "jost",
        name: "Jost",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "Jost-Regular.ttf"),
            (Style::Bold, "Jost-Bold.ttf"),
            (Style::Italic, "Jost-Italic.ttf"),
            (Style::BoldItalic, "Jost-BoldItalic.ttf"),
        ],
    },
    BundledFamily {
        id: "besley",
        name: "Besley",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "Besley-Regular.ttf"),
            (Style::Bold, "Besley-Bold.ttf"),
            (Style::Italic, "Besley-Italic.ttf"),
            (Style::BoldItalic, "Besley-BoldItalic.ttf"),
        ],
    },
    BundledFamily {
        id: "libre-baskerville",
        name: "Libre Baskerville",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "LibreBaskerville-Regular.ttf"),
            (Style::Bold, "LibreBaskerville-Bold.ttf"),
            (Style::Italic, "LibreBaskerville-Italic.ttf"),
        ],
    },
    BundledFamily {
        id: "cinzel",
        name: "Cinzel",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "Cinzel-Regular.ttf"),
            (Style::Bold, "Cinzel-Bold.ttf"),
        ],
    },
    BundledFamily {
        id: "cinzel-decorative",
        name: "Cinzel Decorative",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "CinzelDecorative-Regular.ttf"),
            (Style::Bold, "CinzelDecorative-Bold.ttf"),
        ],
    },
    BundledFamily {
        id: "barlow",
        name: "Barlow",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "Barlow-Regular.ttf"),
            (Style::Bold, "Barlow-Bold.ttf"),
            (Style::Italic, "Barlow-Italic.ttf"),
            (Style::BoldItalic, "Barlow-BoldItalic.ttf"),
        ],
    },
    BundledFamily {
        id: "barlow-condensed",
        name: "Barlow Condensed",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "BarlowCondensed-Regular.ttf"),
            (Style::Bold, "BarlowCondensed-Bold.ttf"),
            (Style::Italic, "BarlowCondensed-Italic.ttf"),
            (Style::BoldItalic, "BarlowCondensed-BoldItalic.ttf"),
        ],
    },
    BundledFamily {
        id: "varela-round",
        name: "Varela Round",
        licence: "SIL OFL 1.1",
        faces: &[(Style::Regular, "VarelaRound-Regular.ttf")],
    },
    BundledFamily {
        id: "charis-sil",
        name: "Charis SIL",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "CharisSIL-Regular.ttf"),
            (Style::Bold, "CharisSIL-Bold.ttf"),
            (Style::Italic, "CharisSIL-Italic.ttf"),
            (Style::BoldItalic, "CharisSIL-BoldItalic.ttf"),
        ],
    },
    BundledFamily {
        id: "comic-neue",
        name: "Comic Neue",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "ComicNeue-Regular.ttf"),
            (Style::Bold, "ComicNeue-Bold.ttf"),
            (Style::Italic, "ComicNeue-Italic.ttf"),
            (Style::BoldItalic, "ComicNeue-BoldItalic.ttf"),
        ],
    },
    BundledFamily {
        id: "cabin-sketch",
        name: "Cabin Sketch",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "CabinSketch-Regular.ttf"),
            (Style::Bold, "CabinSketch-Bold.ttf"),
        ],
    },
    BundledFamily {
        id: "patrick-hand",
        name: "Patrick Hand",
        licence: "SIL OFL 1.1",
        faces: &[(Style::Regular, "PatrickHand-Regular.ttf")],
    },
    BundledFamily {
        id: "kalam",
        name: "Kalam",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "Kalam-Regular.ttf"),
            (Style::Bold, "Kalam-Bold.ttf"),
        ],
    },
    BundledFamily {
        id: "allura",
        name: "Allura",
        licence: "SIL OFL 1.1",
        faces: &[(Style::Regular, "Allura-Regular.ttf")],
    },
    BundledFamily {
        id: "alex-brush",
        name: "Alex Brush",
        licence: "SIL OFL 1.1",
        faces: &[(Style::Regular, "AlexBrush-Regular.ttf")],
    },
    BundledFamily {
        id: "tangerine",
        name: "Tangerine",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "Tangerine-Regular.ttf"),
            (Style::Bold, "Tangerine-Bold.ttf"),
        ],
    },
    BundledFamily {
        id: "almendra",
        name: "Almendra",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "Almendra-Regular.ttf"),
            (Style::Bold, "Almendra-Bold.ttf"),
            (Style::Italic, "Almendra-Italic.ttf"),
            (Style::BoldItalic, "Almendra-BoldItalic.ttf"),
        ],
    },
    BundledFamily {
        id: "lobster",
        name: "Lobster",
        licence: "SIL OFL 1.1",
        faces: &[(Style::Regular, "Lobster-Regular.ttf")],
    },
    BundledFamily {
        id: "cutive",
        name: "Cutive",
        licence: "SIL OFL 1.1",
        faces: &[(Style::Regular, "Cutive-Regular.ttf")],
    },
    BundledFamily {
        id: "noto-sans-symbols-2",
        name: "Noto Sans Symbols 2",
        licence: "SIL OFL 1.1",
        faces: &[(Style::Regular, "NotoSansSymbols2-Regular.ttf")],
    },
    BundledFamily {
        id: "noto-sans-jp",
        name: "Noto Sans JP",
        licence: "SIL OFL 1.1",
        faces: &[
            (Style::Regular, "NotoSansJP-Regular.otf"),
            (Style::Bold, "NotoSansJP-Bold.otf"),
        ],
    },
];

/// Every iPhone font tapHLE knows what to do with, and what it draws instead.
///
/// The list is the public font families of the iPhone OS era. A family absent
/// from it is not refused — it falls back to a sans, serif or monospace by the
/// shape of its name — but it gets no considered answer, which is what being
/// here means.
pub const SUBSTITUTIONS: &[Substitution] = &[
    // The three metric substitutions, which are the reason a layout still fits.
    Substitution {
        ios_family: "Arial",
        aliases: &[],
        bundled: "liberation-sans",
        closeness: Closeness::Metric,
        note: "Drawn to Arial's metrics.",
    },
    Substitution {
        ios_family: "Times New Roman",
        aliases: &[],
        bundled: "liberation-serif",
        closeness: Closeness::Metric,
        note: "Drawn to Times New Roman's metrics.",
    },
    Substitution {
        ios_family: "Courier New",
        aliases: &[],
        bundled: "liberation-mono",
        closeness: Closeness::Metric,
        note: "Drawn to Courier New's metrics.",
    },
    Substitution {
        ios_family: "Georgia",
        aliases: &[],
        bundled: "gelasio",
        closeness: Closeness::Metric,
        note: "Drawn to Georgia's metrics.",
    },
    // Helvetica and its relatives. Metric rather than closest-looking, on
    // purpose: see the note at the top of this file.
    Substitution {
        ios_family: "Helvetica",
        aliases: &[],
        bundled: "liberation-sans",
        closeness: Closeness::Metric,
        note: "By way of Arial's metrics, so text still fits.",
    },
    Substitution {
        ios_family: "Helvetica Neue",
        aliases: &["HelveticaNeue"],
        bundled: "liberation-sans",
        closeness: Closeness::Metric,
        note: "As Helvetica. Inter looks closer.",
    },
    Substitution {
        ios_family: "Helvetica Neue Condensed",
        aliases: &["HelveticaNeueCondensed"],
        bundled: "barlow-condensed",
        closeness: Closeness::Role,
        note: "A condensed grotesque in the same role.",
    },
    Substitution {
        ios_family: "Courier",
        aliases: &[],
        bundled: "liberation-mono",
        closeness: Closeness::Metric,
        note: "Courier New's metrics, which Courier shares.",
    },
    Substitution {
        ios_family: "Verdana",
        aliases: &[],
        bundled: "liberation-sans",
        closeness: Closeness::Shape,
        note: "Verdana is wider; text will take less room.",
    },
    Substitution {
        ios_family: "Trebuchet MS",
        aliases: &[],
        bundled: "liberation-sans",
        closeness: Closeness::Shape,
        note: "A humanist face for a humanist face.",
    },
    // The system-font era. These barely appear in 32-bit apps, but an app that
    // asks by name should get the intended answer.
    Substitution {
        ios_family: "San Francisco",
        aliases: &["SFUIText", "SFUIDisplay", "SFProText", "SFProDisplay"],
        bundled: "inter",
        closeness: Closeness::Shape,
        note: "The closest open face to San Francisco.",
    },
    Substitution {
        ios_family: "Avenir",
        aliases: &[],
        bundled: "inter",
        closeness: Closeness::Role,
        note: "No open Avenir exists; Inter fills the role.",
    },
    Substitution {
        ios_family: "Avenir Next",
        aliases: &["AvenirNext"],
        bundled: "inter",
        closeness: Closeness::Role,
        note: "As Avenir.",
    },
    Substitution {
        ios_family: "Avenir Next Condensed",
        aliases: &["AvenirNextCondensed"],
        bundled: "barlow-condensed",
        closeness: Closeness::Role,
        note: "A condensed grotesque for a geometric.",
    },
    // Text and display serifs.
    Substitution {
        ios_family: "Baskerville",
        aliases: &[],
        bundled: "libre-baskerville",
        closeness: Closeness::Shape,
        note: "A Baskerville revival drawn for screens; wider.",
    },
    Substitution {
        ios_family: "Hoefler Text",
        aliases: &["HoeflerText"],
        bundled: "charis-sil",
        closeness: Closeness::Role,
        note: "An old-style text serif.",
    },
    Substitution {
        ios_family: "Iowan Old Style",
        aliases: &["IowanOldStyle"],
        bundled: "charis-sil",
        closeness: Closeness::Role,
        note: "An old-style text serif.",
    },
    Substitution {
        ios_family: "Marion",
        aliases: &[],
        bundled: "charis-sil",
        closeness: Closeness::Role,
        note: "A reading serif.",
    },
    Substitution {
        ios_family: "Palatino",
        aliases: &[],
        bundled: "charis-sil",
        closeness: Closeness::Role,
        note: "A humanist book serif.",
    },
    Substitution {
        ios_family: "Cochin",
        aliases: &[],
        bundled: "libre-baskerville",
        closeness: Closeness::Role,
        note: "An old-style display serif.",
    },
    Substitution {
        ios_family: "Didot",
        aliases: &[],
        bundled: "libre-baskerville",
        closeness: Closeness::Role,
        note: "Not a Didone; much less contrast.",
    },
    Substitution {
        ios_family: "Bodoni 72",
        aliases: &["Bodoni72", "BodoniSvtyTwoITCTT", "BodoniOrnamentsITCTT"],
        bundled: "libre-baskerville",
        closeness: Closeness::Role,
        note: "Not a Didone; much less contrast.",
    },
    Substitution {
        ios_family: "Superclarendon",
        aliases: &[],
        bundled: "besley",
        closeness: Closeness::Shape,
        note: "An open Clarendon.",
    },
    Substitution {
        ios_family: "American Typewriter",
        aliases: &["AmericanTypewriter"],
        bundled: "cutive",
        closeness: Closeness::Role,
        note: "A typewriter slab.",
    },
    // Geometric and industrial sans.
    Substitution {
        ios_family: "Futura",
        aliases: &[],
        bundled: "jost",
        closeness: Closeness::Shape,
        note: "Drawn after the same 1920s geometric sans.",
    },
    Substitution {
        ios_family: "Gill Sans",
        aliases: &["GillSans"],
        bundled: "barlow",
        closeness: Closeness::Role,
        note: "A humanist sans in the same role.",
    },
    Substitution {
        ios_family: "Optima",
        aliases: &[],
        bundled: "barlow",
        closeness: Closeness::Role,
        note: "Optima's flared stems have no open match.",
    },
    Substitution {
        ios_family: "DIN Alternate",
        aliases: &["DINAlternate"],
        bundled: "barlow",
        closeness: Closeness::Role,
        note: "An industrial grotesque.",
    },
    Substitution {
        ios_family: "DIN Condensed",
        aliases: &["DINCondensed"],
        bundled: "barlow-condensed",
        closeness: Closeness::Role,
        note: "A condensed industrial grotesque.",
    },
    Substitution {
        ios_family: "Arial Rounded MT Bold",
        aliases: &["ArialRoundedMTBold"],
        bundled: "varela-round",
        closeness: Closeness::Role,
        note: "A rounded sans; one weight only.",
    },
    Substitution {
        ios_family: "Copperplate",
        aliases: &[],
        bundled: "cinzel",
        closeness: Closeness::Role,
        note: "An inscriptional capital face.",
    },
    Substitution {
        ios_family: "Academy Engraved LET",
        aliases: &["AcademyEngravedLetPlain"],
        bundled: "cinzel-decorative",
        closeness: Closeness::Role,
        note: "A decorative engraved face.",
    },
    // Handwriting, chalk and script.
    Substitution {
        ios_family: "Marker Felt",
        aliases: &["MarkerFelt"],
        bundled: "patrick-hand",
        closeness: Closeness::Role,
        note: "A handwriting face.",
    },
    Substitution {
        ios_family: "Bradley Hand",
        aliases: &["BradleyHandITCTT"],
        bundled: "patrick-hand",
        closeness: Closeness::Role,
        note: "A handwriting face.",
    },
    Substitution {
        ios_family: "Noteworthy",
        aliases: &[],
        bundled: "kalam",
        closeness: Closeness::Role,
        note: "A handwriting face, with a bold.",
    },
    Substitution {
        ios_family: "Chalkboard SE",
        aliases: &["ChalkboardSE"],
        bundled: "comic-neue",
        closeness: Closeness::Role,
        note: "An informal sans.",
    },
    Substitution {
        ios_family: "Chalkduster",
        aliases: &[],
        bundled: "cabin-sketch",
        closeness: Closeness::Role,
        note: "A sketched face in the same role, not chalk.",
    },
    Substitution {
        ios_family: "Snell Roundhand",
        aliases: &["SnellRoundhand"],
        bundled: "allura",
        closeness: Closeness::Role,
        note: "A formal script in the same role; widths differ a lot.",
    },
    Substitution {
        ios_family: "Savoye LET",
        aliases: &["SavoyeLetPlain"],
        bundled: "alex-brush",
        closeness: Closeness::Role,
        note: "A brush script in the same role; widths differ a lot.",
    },
    Substitution {
        ios_family: "Zapfino",
        aliases: &[],
        bundled: "tangerine",
        closeness: Closeness::Role,
        note: "Nothing open is close to Zapfino. Much narrower.",
    },
    Substitution {
        ios_family: "Papyrus",
        aliases: &[],
        bundled: "almendra",
        closeness: Closeness::Role,
        note: "Nothing open is close to Papyrus.",
    },
    Substitution {
        ios_family: "Party LET",
        aliases: &["PartyLetPlain"],
        bundled: "lobster",
        closeness: Closeness::Role,
        note: "Nothing open is close to Party LET.",
    },
    // Symbols and Japanese.
    Substitution {
        ios_family: "Symbol",
        aliases: &[],
        bundled: "noto-sans-symbols-2",
        closeness: Closeness::Role,
        note: "Symbols by Unicode meaning, not the old code points.",
    },
    Substitution {
        ios_family: "Zapf Dingbats",
        aliases: &["ZapfDingbatsITC"],
        bundled: "noto-sans-symbols-2",
        closeness: Closeness::Role,
        note: "Symbols by Unicode meaning, not the old code points.",
    },
    Substitution {
        ios_family: "Hiragino Kaku Gothic ProN",
        aliases: &["HiraKakuProN", "HiraKakuPro", "HiraginoSans"],
        bundled: "noto-sans-jp",
        closeness: Closeness::Role,
        note: "A Japanese gothic.",
    },
    Substitution {
        ios_family: "Hiragino Mincho ProN",
        aliases: &["HiraMinProN", "HiraMinPro"],
        bundled: "noto-sans-jp",
        closeness: Closeness::Role,
        note: "A gothic for a mincho: the serifs are missing.",
    },
];

/// A font tapHLE has decided to draw, and where it comes from.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Face {
    /// One of the families tapHLE ships.
    Bundled { family: &'static str, style: Style },
    /// A font file on this computer: either the app's own font by its real
    /// name, or one the person chose in the frontend.
    Host { path: PathBuf, index: u32 },
}

/// Find a bundled family by its catalogue id.
pub fn bundled(id: &str) -> Option<&'static BundledFamily> {
    BUNDLED.iter().find(|family| family.id == id)
}

/// Find what stands in for an iPhone family, by the family's own name.
pub fn substitution(ios_family: &str) -> Option<&'static Substitution> {
    let key = normalise(ios_family);
    SUBSTITUTIONS.iter().find(|substitution| {
        normalise(substitution.ios_family) == key
            || substitution
                .aliases
                .iter()
                .any(|alias| normalise(alias) == key)
    })
}

/// Style words and foundry suffixes that are not part of a family's identity.
///
/// Longest first, so that "boldItalic" is consumed before "bold" leaves an
/// "italic" behind, and "extralight" before "light".
///
/// "Roman" is deliberately absent. It is a style word — some families call
/// their upright face that — but it is also the last word of Times New Roman,
/// and stripping it turned the most common serif on the platform into
/// "timesnew", which matched nothing.
const NOT_PART_OF_THE_FAMILY: &[&str] = &[
    "ultralightitalic",
    "extralightitalic",
    "semibolditalic",
    "demibolditalic",
    "boldoblique",
    "extralight",
    "ultralight",
    "lightitalic",
    "mediumitalic",
    "blackitalic",
    "heavyitalic",
    "thinitalic",
    "bookitalic",
    "bolditalic",
    "semibold",
    "demibold",
    "oblique",
    "regular",
    "italic",
    "medium",
    "black",
    "heavy",
    "light",
    "plain",
    "book",
    "bold",
    "thin",
    "psmt",
    "itc",
    "tt",
    "ps",
    "mt",
];

/// Reduce a font name to the family it belongs to.
///
/// An app names a face — `TimesNewRomanPS-BoldMT`, `HelveticaNeue-Bold`,
/// `MarkerFelt-Thin` — and what the catalogue is keyed by is the family. The
/// alternative, listing every PostScript name Apple ever shipped, is what this
/// replaced: it was 130 lines long, it was wrong about several of them, and any
/// name not in it fell through to the system font in silence.
pub fn normalise(name: &str) -> String {
    let mut key: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();

    // Weight numbers Apple's Japanese faces use, as in `HiraKakuProN-W6`.
    for weight in ['1', '2', '3', '4', '5', '6', '7', '8', '9'] {
        let suffix = format!("w{weight}");
        if let Some(trimmed) = key.strip_suffix(&suffix) {
            key = trimmed.to_string();
            break;
        }
    }

    loop {
        let before = key.len();
        for word in NOT_PART_OF_THE_FAMILY {
            if key.len() > word.len() {
                if let Some(trimmed) = key.strip_suffix(word) {
                    key = trimmed.to_string();
                    break;
                }
            }
        }
        if key.len() == before {
            return key;
        }
    }
}

/// The style a font name asks for.
pub fn style_of(name: &str) -> Style {
    let lower = name.to_ascii_lowercase();
    let bold = lower.contains("bold")
        || lower.contains("black")
        || lower.contains("heavy")
        || lower.contains("-w6")
        || lower.contains("-w7");
    let italic = lower.contains("italic") || lower.contains("oblique");
    Style::new(bold, italic)
}

/// The generic family for a name the catalogue has no entry for.
///
/// The same coarse guess tapHLE has always made, kept for exactly the case it
/// was always for: a font nobody has classified.
pub fn generic_family(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.contains("courier")
        || lower.contains("mono")
        || lower.contains("typewriter")
        || lower.contains("consol")
    {
        "liberation-mono"
    } else if lower.contains("times")
        || lower.contains("serif")
        || lower.contains("georgia")
        || lower.contains("roman")
        || lower.contains("garamond")
        || lower.contains("book")
    {
        "liberation-serif"
    } else {
        "liberation-sans"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_substitution_names_a_family_that_exists() {
        for substitution in SUBSTITUTIONS {
            assert!(
                bundled(substitution.bundled).is_some(),
                "{} substitutes {}, which is not bundled",
                substitution.ios_family,
                substitution.bundled
            );
        }
    }

    /// Falling back to the nearest style relies on there always being a regular
    /// to fall back to.
    #[test]
    fn every_bundled_family_has_a_regular() {
        for family in BUNDLED {
            assert!(
                family.has_style(Style::Regular),
                "{} has no regular face",
                family.id
            );
        }
    }

    #[test]
    fn no_two_bundled_families_share_an_id() {
        for (index, family) in BUNDLED.iter().enumerate() {
            assert!(
                !BUNDLED[..index].iter().any(|other| other.id == family.id),
                "two families are called {}",
                family.id
            );
        }
    }

    /// Two iPhone families must not normalise to the same key, or one of them
    /// silently answers for the other.
    #[test]
    fn no_two_ios_families_normalise_alike() {
        for (index, substitution) in SUBSTITUTIONS.iter().enumerate() {
            let key = normalise(substitution.ios_family);
            assert!(
                !SUBSTITUTIONS[..index]
                    .iter()
                    .any(|other| normalise(other.ios_family) == key),
                "{} normalises to {:?}, which is already taken",
                substitution.ios_family,
                key
            );
        }
    }

    /// The PostScript names apps actually pass, and the family each belongs to.
    #[test]
    fn postscript_names_reduce_to_their_family() {
        let cases = [
            ("ArialMT", "arial"),
            ("Arial-BoldMT", "arial"),
            ("Arial-BoldItalicMT", "arial"),
            ("TimesNewRomanPSMT", "timesnewroman"),
            ("TimesNewRomanPS-BoldMT", "timesnewroman"),
            ("TimesNewRomanPS-BoldItalicMT", "timesnewroman"),
            ("CourierNewPSMT", "couriernew"),
            ("CourierNewPS-BoldMT", "couriernew"),
            ("Helvetica", "helvetica"),
            ("Helvetica-BoldOblique", "helvetica"),
            ("HelveticaNeue-Bold", "helveticaneue"),
            ("MarkerFelt-Thin", "markerfelt"),
            ("Zapfino", "zapfino"),
            ("Georgia-BoldItalic", "georgia"),
            ("AmericanTypewriter-Bold", "americantypewriter"),
            ("HiraKakuProN-W6", "hirakakupron"),
            ("Baskerville-SemiBold", "baskerville"),
            ("ChalkboardSE-Regular", "chalkboardse"),
            ("Verdana-BoldItalic", "verdana"),
        ];
        for (name, expected) in cases {
            assert_eq!(normalise(name), expected, "normalising {name}");
        }
    }

    /// Every one of those names has to reach a substitution, which is the
    /// whole point of reducing them.
    #[test]
    fn the_names_apps_use_find_a_substitution() {
        let names = [
            "ArialMT",
            "Arial-BoldMT",
            "TimesNewRomanPSMT",
            "CourierNewPSMT",
            "Helvetica",
            "Helvetica-BoldOblique",
            "HelveticaNeue-Bold",
            "MarkerFelt-Thin",
            "Zapfino",
            "Georgia-Bold",
            "AmericanTypewriter",
            "HiraKakuProN-W6",
            "Baskerville",
            "ChalkboardSE-Light",
            "Verdana",
            "Futura-Medium",
            "Papyrus",
            "SnellRoundhand-Black",
            "Copperplate-Bold",
            "GillSans-Italic",
        ];
        for name in names {
            assert!(
                substitution(name).is_some(),
                "{name} does not reach a substitution"
            );
        }
    }

    #[test]
    fn the_style_is_read_from_the_name() {
        assert_eq!(style_of("Helvetica"), Style::Regular);
        assert_eq!(style_of("Helvetica-Bold"), Style::Bold);
        assert_eq!(style_of("Helvetica-Oblique"), Style::Italic);
        assert_eq!(style_of("Helvetica-BoldOblique"), Style::BoldItalic);
        assert_eq!(style_of("Arial-BoldItalicMT"), Style::BoldItalic);
        assert_eq!(style_of("HiraKakuProN-W6"), Style::Bold);
        assert_eq!(style_of("HiraKakuProN-W3"), Style::Regular);
    }

    /// A family with fewer than four faces still answers for all four.
    #[test]
    fn a_missing_style_falls_back_within_the_family() {
        let round = bundled("varela-round").unwrap();
        assert_eq!(round.file_for(Style::Regular), "VarelaRound-Regular.ttf");
        assert_eq!(round.file_for(Style::BoldItalic), "VarelaRound-Regular.ttf");

        let cinzel = bundled("cinzel").unwrap();
        assert_eq!(cinzel.file_for(Style::Bold), "Cinzel-Bold.ttf");
        // No italic, so the weight is kept and the slant dropped.
        assert_eq!(cinzel.file_for(Style::BoldItalic), "Cinzel-Bold.ttf");
        assert_eq!(cinzel.file_for(Style::Italic), "Cinzel-Regular.ttf");
    }
}
