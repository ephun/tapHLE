# Fonts

tapHLE cannot ship the fonts an iPhone has. Helvetica, Arial, Futura, Zapfino,
PingFang and almost every other font on the device are commercially licensed,
and a copy being present on a phone grants nothing about redistributing it —
Apple's own San Francisco licence says in as many words that the font may not
be embedded in software.

So this directory holds open substitutes, and `src/font/catalogue.rs` says
which stands in for which and how close the match actually is. Three kinds of
closeness are distinguished there, because conflating them is how a
substitution table stops being trustworthy:

- **Metric** — text takes the same space, so a layout built around the original
  still fits. Liberation Sans for Arial, Liberation Serif for Times New Roman,
  Liberation Mono for Courier New, Gelasio for Georgia.
- **Shape** — similar construction, different widths. Text reflows.
- **Role** — the same job, a different typeface. Widths are not comparable.

An installed copy of the real font is preferred over any of this: tapHLE looks
for the font an app asks for among the ones installed on the computer before
falling back to a substitute, and a person can pick a font per iPhone font in
the frontend.

## Adding to or refreshing this directory

    python dev-scripts/fetch-fonts.py            # fetch anything missing
    python dev-scripts/fetch-fonts.py --check    # verify what is here

The script records where every file came from, pins its SHA-256, and refuses a
download whose `name` table does not report the family and style expected of
it. Its manifest is the list of what is here; this file is prose about it.

Static faces only: tapHLE's rasteriser draws a variable font's default instance
and cannot move an axis, so a variable-only family would give one weight
wearing four names.

## What is here, and under what licence

Every family is under the SIL Open Font License 1.1, which permits
redistribution inside a program. Each family's licence text is in `licenses/`,
which the OFL requires.

| Family | Stands in for |
| --- | --- |
| Liberation Sans / Serif / Mono | Arial, Times New Roman, Courier New, Helvetica |
| Inter | San Francisco, Avenir, Avenir Next |
| Gelasio | Georgia |
| Jost | Futura |
| Besley | Superclarendon |
| Libre Baskerville | Baskerville, Cochin, Didot, Bodoni 72 |
| Charis SIL | Iowan Old Style, Hoefler Text, Palatino, Marion |
| Cinzel / Cinzel Decorative | Copperplate, Academy Engraved LET |
| Barlow / Barlow Condensed | DIN Alternate, DIN Condensed, Gill Sans, Optima, Avenir Next Condensed |
| Varela Round | Arial Rounded MT Bold |
| Cutive | American Typewriter |
| Comic Neue | Chalkboard SE |
| Cabin Sketch | Chalkduster |
| Patrick Hand / Kalam | Marker Felt, Bradley Hand, Noteworthy |
| Allura / Alex Brush | Snell Roundhand, Savoye LET |
| Tangerine | Zapfino |
| Almendra | Papyrus |
| Lobster | Party LET |
| Noto Sans Symbols 2 | Symbol, Zapf Dingbats |
| Noto Sans JP | Hiragino Kaku Gothic ProN, Hiragino Mincho ProN |

The last four rows are role substitutes and nothing more. Nothing open is close
to Zapfino, Papyrus or Party LET; the catalogue says so in the note it shows
beside each, rather than implying a resemblance that is not there.

## Scripts other than Latin and Japanese

Not bundled yet. The Noto families cover Chinese, Korean, Arabic, Hebrew, Thai
and the Indic scripts, and they are the right answer, but a full set is more
than a hundred megabytes and would sit in this repository's history for good.
An app needing one of those scripts still gets the right font if the computer
running tapHLE has it installed, because the host-font lookup happens first.

## Earlier provenance

The Liberation fonts came from release 2.1.5 and the Noto Sans JP files from
the Noto Sans CJK release current on 2023-01-28, both predating the fetch
script; their licences are `LICENSE.liberation` and `LICENSE.noto`.
