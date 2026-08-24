#!/usr/bin/env python3
"""Download the permissively licensed fonts tapHLE ships, and check them.

tapHLE cannot ship the fonts an iPhone has. Almost every one of them —
Helvetica, Arial, Futura, Zapfino, PingFang — is commercially licensed, and
having a copy on a device grants nothing about redistributing it. What tapHLE
ships instead is a set of open substitutes, chosen for how close they come to
the original's role and, where such a thing exists, its metrics.

This script fetches them. It exists so that the provenance of every file in
`runtime/fonts/` is written down rather than remembered: where it came from,
what it hashes to, and under which licence.

    python dev-scripts/fetch-fonts.py            # fetch anything missing
    python dev-scripts/fetch-fonts.py --check    # verify what is there
    python dev-scripts/fetch-fonts.py --force    # fetch everything again

Every download is verified before it is kept:

- the bytes must hash to the recorded SHA-256, once one is recorded;
- the file must parse far enough to read its `name` table;
- that table must report the family and subfamily expected of it.

A file that fails any of these is not written. Recording the hash of whatever
happened to arrive would defeat the purpose, so a file with no hash yet is
fetched, checked by name, and its hash printed to be pinned deliberately.

## On licences

Only OFL 1.1 and Apache 2.0 families are listed here. Both permit
redistribution inside a program, which is the whole question — a font being
present on a device says nothing about the right to copy it. Each family's
licence is fetched alongside it into `runtime/fonts/licenses/`, which the OFL
requires rather than merely encourages.

## Why some families are here and better-known ones are not

Static faces only. tapHLE's rasteriser draws a variable font's default
instance and has no way to move an axis, so a variable-only family would give
one weight wearing four names. Where a family publishes statics upstream but
only a variable file on Google Fonts, upstream is used.
"""

import argparse
import hashlib
import io
import os
import struct
import sys
import urllib.request
import zipfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FONTS = os.path.join(REPO, 'runtime', 'fonts')
LICENSES = os.path.join(FONTS, 'licenses')

RAW = 'https://raw.githubusercontent.com'
GF = RAW + '/google/fonts/main'


def gf(directory, filename):
    return '%s/%s/%s' % (GF, directory, filename)


# Each family says where its files come from and what each one must turn out to
# be. The (family, subfamily) pair is read from the downloaded font's own name
# table and compared, which is what catches a moved file, a truncated download
# or a variable font standing in for a static one.
#
# `substitutes` names the iPhone fonts a family is here for, and nothing more.
# The mapping the emulator uses, and how close each match is, live in
# `src/font/catalogue.rs`, because they have to be compiled in.
FAMILIES = [
    {
        'id': 'inter', 'name': 'Inter', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/rsms/inter/master/LICENSE.txt',
        'substitutes': 'San Francisco, Avenir, Avenir Next',
        'archive': 'https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip',
        'files': {
            'Inter-Bold.ttf': (
                'extras/ttf/Inter-Bold.ttf',
                ('Inter', 'Bold')),
            'Inter-BoldItalic.ttf': (
                'extras/ttf/Inter-BoldItalic.ttf',
                ('Inter', 'Bold Italic')),
            'Inter-Italic.ttf': (
                'extras/ttf/Inter-Italic.ttf',
                ('Inter', 'Italic')),
            'Inter-Regular.ttf': (
                'extras/ttf/Inter-Regular.ttf',
                ('Inter', 'Regular')),
        },
        'sha256': {
            'Inter-Bold.ttf': '288316099b1e0a47a4716d159098005eef7c0066921f34e3200393dbdb01947f',
            'Inter-BoldItalic.ttf': '948405a16cdc62701da5f4005ed068ca5f4d27061d98f7974ccfc37831d9581d',
            'Inter-Italic.ttf': 'bbc051dd204b5019a1aa0bc0ae2aa8a05ab13e7a3f979fa357631dc7feb6833a',
            'Inter-Regular.ttf': '40d692fce188e4471e2b3cba937be967878f631ad3ebbbdcd587687c7ebe0c82',
        },
    },
    {
        'id': 'gelasio', 'name': 'Gelasio', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/gelasio/OFL.txt',
        'substitutes': 'Georgia',
        'files': {
            'Gelasio-Bold.ttf': (
                'https://raw.githubusercontent.com/SorkinType/Gelasio/master/fonts/ttf/Gelasio-Bold.ttf',
                ('Gelasio', 'Bold')),
            'Gelasio-BoldItalic.ttf': (
                'https://raw.githubusercontent.com/SorkinType/Gelasio/master/fonts/ttf/Gelasio-BoldItalic.ttf',
                ('Gelasio', 'Bold Italic')),
            'Gelasio-Italic.ttf': (
                'https://raw.githubusercontent.com/SorkinType/Gelasio/master/fonts/ttf/Gelasio-Italic.ttf',
                ('Gelasio', 'Italic')),
            'Gelasio-Regular.ttf': (
                'https://raw.githubusercontent.com/SorkinType/Gelasio/master/fonts/ttf/Gelasio-Regular.ttf',
                ('Gelasio', 'Regular')),
        },
        'sha256': {
            'Gelasio-Bold.ttf': 'e0ef3addf1acf35f5c6aef2be00d0a2c01363bf70a5950e16650976051a0c462',
            'Gelasio-BoldItalic.ttf': 'aa87e21b46aef3dcb50a68dc6e1946f0c0a9819d067e4be6a98a6daa8f5b3c16',
            'Gelasio-Italic.ttf': '1d8bda4ff258bb8251eae4f7d7500c8d97d084136192bb264bc2305168765d1b',
            'Gelasio-Regular.ttf': '48c797fbe0e07c48a18cb962e7bdfa23f19618327dddf54093265328dc9eb39d',
        },
    },
    {
        'id': 'jost', 'name': 'Jost', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/indestructible-type/Jost/master/OFL.txt',
        'substitutes': 'Futura',
        'files': {
            'Jost-Bold.ttf': (
                'https://raw.githubusercontent.com/indestructible-type/Jost/master/fonts/ttf/Jost-700-Bold.ttf',
                ('Jost*', 'Bold')),
            'Jost-BoldItalic.ttf': (
                'https://raw.githubusercontent.com/indestructible-type/Jost/master/fonts/ttf/Jost-700-BoldItalic.ttf',
                ('Jost*', 'Bold Italic')),
            'Jost-Italic.ttf': (
                'https://raw.githubusercontent.com/indestructible-type/Jost/master/fonts/ttf/Jost-400-BookItalic.ttf',
                ('Jost*', 'Book Italic')),
            'Jost-Regular.ttf': (
                'https://raw.githubusercontent.com/indestructible-type/Jost/master/fonts/ttf/Jost-400-Book.ttf',
                ('Jost*', 'Book')),
        },
        'sha256': {
            'Jost-Bold.ttf': '95f16d07698549232a121443d9fdef129de15b209d70c8804f28df49bf594838',
            'Jost-BoldItalic.ttf': '5430d1114cc0267f56908eae988196f4a1ed4d55adce02931be4181de0ae25ca',
            'Jost-Italic.ttf': '295fdb0fe7d66e10480813b619499d14516af20878aaff0246dc28377713cefa',
            'Jost-Regular.ttf': '3fda07bcb2fe66667798f45dd2fc3834586a15fc6b6df7da36422b459b5a117c',
        },
    },
    {
        'id': 'besley', 'name': 'Besley', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/indestructible-type/Besley/master/OFL.txt',
        'substitutes': 'Superclarendon',
        'files': {
            'Besley-Bold.ttf': (
                'https://raw.githubusercontent.com/indestructible-type/Besley/master/fonts/ttf/Besley-Bold.ttf',
                ('Besley', 'Bold')),
            'Besley-BoldItalic.ttf': (
                'https://raw.githubusercontent.com/indestructible-type/Besley/master/fonts/ttf/Besley-BoldItalic.ttf',
                ('Besley', 'Bold Italic')),
            'Besley-Italic.ttf': (
                'https://raw.githubusercontent.com/indestructible-type/Besley/master/fonts/ttf/Besley-Italic.ttf',
                ('Besley', 'Italic')),
            'Besley-Regular.ttf': (
                'https://raw.githubusercontent.com/indestructible-type/Besley/master/fonts/ttf/Besley-Regular.ttf',
                ('Besley', 'Regular')),
        },
        'sha256': {
            'Besley-Bold.ttf': '9310332b177198034adc79e9f87c1bff6e3ae53de9ec207973ae4e5799abd35e',
            'Besley-BoldItalic.ttf': 'a61ecc7923f35b544f210222cbf7d72f3ebfe4b6ec855aeee5e2545054b73d2f',
            'Besley-Italic.ttf': '8300f5f6346d4d8312623accb0c5bb30c2b47df1bd28d0e6492bd46c2ea1f2ac',
            'Besley-Regular.ttf': '0a94bdff8dc5eb489b5e425ced6f45109501d91937ebcc6a1ebe7e4b579233b1',
        },
    },
    {
        'id': 'librebaskerville', 'name': 'Libre Baskerville', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/librebaskerville/OFL.txt',
        'substitutes': 'Baskerville',
        'files': {
            'LibreBaskerville-Bold.ttf': (
                'https://raw.githubusercontent.com/impallari/Libre-Baskerville/master/fonts/ttf/LibreBaskerville-Bold.ttf',
                ('Libre Baskerville', 'Bold')),
            'LibreBaskerville-Italic.ttf': (
                'https://raw.githubusercontent.com/impallari/Libre-Baskerville/master/fonts/ttf/LibreBaskerville-Italic.ttf',
                ('Libre Baskerville', 'Italic')),
            'LibreBaskerville-Regular.ttf': (
                'https://raw.githubusercontent.com/impallari/Libre-Baskerville/master/fonts/ttf/LibreBaskerville-Regular.ttf',
                ('Libre Baskerville', 'Regular')),
        },
        'sha256': {
            'LibreBaskerville-Bold.ttf': '62e23c5e6bf1e68fc0ef4ddeeab894ac21c808027f6cb6730cda2055a159ae03',
            'LibreBaskerville-Italic.ttf': '82d05fab9a1c07f4eb4bc891fb0043a74fba1bed27ea919f61d94dbc5b54a4f6',
            'LibreBaskerville-Regular.ttf': 'df9fddf43dbd7de435c316b86a52b3d6b3ad2f6fb2ed3f6fd8bdc1835f30eec1',
        },
    },
    {
        'id': 'cinzel', 'name': 'Cinzel', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/NDISCOVER/Cinzel/master/OFL.txt',
        'substitutes': 'Copperplate',
        'files': {
            'Cinzel-Bold.ttf': (
                'https://raw.githubusercontent.com/NDISCOVER/Cinzel/master/fonts/ttf/Cinzel-Bold.ttf',
                ('Cinzel', 'Bold')),
            'Cinzel-Regular.ttf': (
                'https://raw.githubusercontent.com/NDISCOVER/Cinzel/master/fonts/ttf/Cinzel-Regular.ttf',
                ('Cinzel', 'Regular')),
        },
        'sha256': {
            'Cinzel-Bold.ttf': '0c23ec565db45c5508ee95889c60ad87debd167ca07167a43a5d68572b4e2eac',
            'Cinzel-Regular.ttf': 'af0031129f27dc752e8629a80b793d27abea94027faa27cc660c3fc33f607a1f',
        },
    },
    {
        'id': 'cinzeldecorative', 'name': 'Cinzel Decorative', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/cinzeldecorative/OFL.txt',
        'substitutes': 'Academy Engraved LET',
        'files': {
            'CinzelDecorative-Bold.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/cinzeldecorative/CinzelDecorative-Bold.ttf',
                ('Cinzel Decorative', 'Bold')),
            'CinzelDecorative-Regular.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/cinzeldecorative/CinzelDecorative-Regular.ttf',
                ('Cinzel Decorative', 'Regular')),
        },
        'sha256': {
            'CinzelDecorative-Bold.ttf': 'e854e68a388aa50d742a4415c1ae5c17a617ef7956c95a70021d0a4a44f20518',
            'CinzelDecorative-Regular.ttf': '5b862be329103ad287a10f0a53e27a40e8cc519999253f1a0223e2dc330b10b8',
        },
    },
    {
        'id': 'barlow', 'name': 'Barlow', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/barlow/OFL.txt',
        'substitutes': 'DIN Alternate',
        'files': {
            'Barlow-Bold.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/barlow/Barlow-Bold.ttf',
                ('Barlow', 'Bold')),
            'Barlow-BoldItalic.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/barlow/Barlow-BoldItalic.ttf',
                ('Barlow', 'Bold Italic')),
            'Barlow-Italic.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/barlow/Barlow-Italic.ttf',
                ('Barlow', 'Italic')),
            'Barlow-Regular.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/barlow/Barlow-Regular.ttf',
                ('Barlow', 'Regular')),
        },
        'sha256': {
            'Barlow-Bold.ttf': '84e6a4d61e7c3e21f3c50ea6a4f7e5303a3467864c038be6ea3759bab8d547f9',
            'Barlow-BoldItalic.ttf': '079dcee4a53544177f3b16354b27b40b521e22861a40084ab4d052f0289ed9e8',
            'Barlow-Italic.ttf': '70cf45c354af39e55082fd506e748cc6a0a1812949875f99ded3f76bf691e4ca',
            'Barlow-Regular.ttf': '95aa02c7c43096e0dd44d787ba6216864a67157e402adab59b35572e0c1577ea',
        },
    },
    {
        'id': 'barlowcondensed', 'name': 'Barlow Condensed', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/barlowcondensed/OFL.txt',
        'substitutes': 'Avenir Next Condensed, DIN Condensed',
        'files': {
            'BarlowCondensed-Bold.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/barlowcondensed/BarlowCondensed-Bold.ttf',
                ('Barlow Condensed', 'Bold')),
            'BarlowCondensed-BoldItalic.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/barlowcondensed/BarlowCondensed-BoldItalic.ttf',
                ('Barlow Condensed', 'Bold Italic')),
            'BarlowCondensed-Italic.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/barlowcondensed/BarlowCondensed-Italic.ttf',
                ('Barlow Condensed', 'Italic')),
            'BarlowCondensed-Regular.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/barlowcondensed/BarlowCondensed-Regular.ttf',
                ('Barlow Condensed', 'Regular')),
        },
        'sha256': {
            'BarlowCondensed-Bold.ttf': 'e476562ec9c1e16cf16475895b511f08c804f438cc9a9f80a44ea50a0eeb5b65',
            'BarlowCondensed-BoldItalic.ttf': 'd8694f65cd70c3e5e600ead37657aab0adb5f63b275d0c1f6f4a7046de222145',
            'BarlowCondensed-Italic.ttf': '063615da0a64f318397de2afac330dfbfbbf4cdb562a9987cb72bfdcbd37ba7c',
            'BarlowCondensed-Regular.ttf': '583cec5da3b84bc4dc7c9c72e2a565c94d34e431518b19d7e250b7830ad5f996',
        },
    },
    {
        'id': 'varelaround', 'name': 'Varela Round', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/varelaround/OFL.txt',
        'substitutes': 'Arial Rounded MT Bold',
        'files': {
            'VarelaRound-Regular.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/varelaround/VarelaRound-Regular.ttf',
                ('Varela Round', 'Regular')),
        },
        'sha256': {
            'VarelaRound-Regular.ttf': 'e1e47eb66dbc2ddc106661338e712d9176c9e83c669a82fde155324823d03aa2',
        },
    },
    {
        'id': 'charissil', 'name': 'Charis SIL', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/charissil/OFL.txt',
        'substitutes': 'Iowan Old Style, Hoefler Text, Marion',
        'files': {
            'CharisSIL-Bold.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/charissil/CharisSIL-Bold.ttf',
                ('Charis SIL', 'Bold')),
            'CharisSIL-BoldItalic.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/charissil/CharisSIL-BoldItalic.ttf',
                ('Charis SIL', 'Bold Italic')),
            'CharisSIL-Italic.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/charissil/CharisSIL-Italic.ttf',
                ('Charis SIL', 'Italic')),
            'CharisSIL-Regular.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/charissil/CharisSIL-Regular.ttf',
                ('Charis SIL', 'Regular')),
        },
        'sha256': {
            'CharisSIL-Bold.ttf': '68460d2b76c8f781b49f9168f2c7dc3c9f7b788a3a1b8f267c5fa6bb47d5c64d',
            'CharisSIL-BoldItalic.ttf': 'acdf6dc54c0ee5e03cae3398f64133c335084cd1ecab655f2a8636c5d56acedb',
            'CharisSIL-Italic.ttf': 'e776e2961117b39cf924c0139de01748a77b2fcb99c437b2c77bdd9401606c13',
            'CharisSIL-Regular.ttf': '346337374aa347d64ee015b26d441f1970d8631914e2f3941b00e1c4761e28c5',
        },
    },
    {
        'id': 'comicneue', 'name': 'Comic Neue', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/comicneue/OFL.txt',
        'substitutes': 'Chalkboard SE',
        'files': {
            'ComicNeue-Bold.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/comicneue/ComicNeue-Bold.ttf',
                ('Comic Neue', 'Bold')),
            'ComicNeue-BoldItalic.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/comicneue/ComicNeue-BoldItalic.ttf',
                ('Comic Neue', 'Bold Italic')),
            'ComicNeue-Italic.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/comicneue/ComicNeue-Italic.ttf',
                ('Comic Neue', 'Italic')),
            'ComicNeue-Regular.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/comicneue/ComicNeue-Regular.ttf',
                ('Comic Neue', 'Regular')),
        },
        'sha256': {
            'ComicNeue-Bold.ttf': '3e7e5fccfd7e0788f317b43312151c1bd5cf058c9697a8d83eac3939050bd61e',
            'ComicNeue-BoldItalic.ttf': '5c312c2a2fa64eee82f3b87fcfab8f3b12a5e59b043124401d322eb323cfbf16',
            'ComicNeue-Italic.ttf': 'e06bfd1552f5c9464c5665733ffd69239b0593885dbb9e059688a5900f78cf98',
            'ComicNeue-Regular.ttf': 'a0ee5a37c8b27c4db0700137d928598b1e23b0089e1546a8961909176b779360',
        },
    },
    {
        'id': 'cabinsketch', 'name': 'Cabin Sketch', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/cabinsketch/OFL.txt',
        'substitutes': 'Chalkduster',
        'files': {
            'CabinSketch-Bold.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/cabinsketch/CabinSketch-Bold.ttf',
                ('Cabin Sketch', 'Bold')),
            'CabinSketch-Regular.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/cabinsketch/CabinSketch-Regular.ttf',
                ('Cabin Sketch', 'Regular')),
        },
        'sha256': {
            'CabinSketch-Bold.ttf': '0961688037b97947495c2e35e6e5a93f2b234490514e33346605ffedf9c7e6be',
            'CabinSketch-Regular.ttf': 'a6f989fcc910ca321e06cd6a20dbcab20f9e41bce084911432857cfe286355e7',
        },
    },
    {
        'id': 'patrickhand', 'name': 'Patrick Hand', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/patrickhand/OFL.txt',
        'substitutes': 'Marker Felt, Bradley Hand, Noteworthy',
        'files': {
            'PatrickHand-Regular.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/patrickhand/PatrickHand-Regular.ttf',
                ('Patrick Hand', 'Regular')),
        },
        'sha256': {
            'PatrickHand-Regular.ttf': '0f173b3e6cb6d1af25babf7f0057c5ac4ee11f9992b0469bb817e967ef4ad0fc',
        },
    },
    {
        'id': 'kalam', 'name': 'Kalam', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/kalam/OFL.txt',
        'substitutes': 'Noteworthy',
        'files': {
            'Kalam-Bold.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/kalam/Kalam-Bold.ttf',
                ('Kalam', 'Bold')),
            'Kalam-Regular.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/kalam/Kalam-Regular.ttf',
                ('Kalam', 'Regular')),
        },
        'sha256': {
            'Kalam-Bold.ttf': '2f6576601db015d4f6c08678120277fc8510b98c06e932ce7a6a9cbff4cbdded',
            'Kalam-Regular.ttf': '57cecb63d4608019371954274ae1d8c397764debd5b19d4a33c1efa4dc923c0b',
        },
    },
    {
        'id': 'allura', 'name': 'Allura', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/allura/OFL.txt',
        'substitutes': 'Snell Roundhand, Savoye LET',
        'files': {
            'Allura-Regular.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/allura/Allura-Regular.ttf',
                ('Allura', 'Regular')),
        },
        'sha256': {
            'Allura-Regular.ttf': '9c142b2e515832c0dfc4ff8b8ea18f40314943bf937b72e2b23c4661bac14cc6',
        },
    },
    {
        'id': 'alexbrush', 'name': 'Alex Brush', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/alexbrush/OFL.txt',
        'substitutes': 'Savoye LET',
        'files': {
            'AlexBrush-Regular.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/alexbrush/AlexBrush-Regular.ttf',
                ('Alex Brush', 'Regular')),
        },
        'sha256': {
            'AlexBrush-Regular.ttf': 'df702038d8e27797230c77959c139eeea38cac0caf53e19ea5b513d3b0d3362d',
        },
    },
    {
        'id': 'tangerine', 'name': 'Tangerine', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/tangerine/OFL.txt',
        'substitutes': 'Zapfino',
        'files': {
            'Tangerine-Bold.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/tangerine/Tangerine-Bold.ttf',
                ('Tangerine', 'Bold')),
            'Tangerine-Regular.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/tangerine/Tangerine-Regular.ttf',
                ('Tangerine', 'Regular')),
        },
        'sha256': {
            'Tangerine-Bold.ttf': 'a35368c814c71c15928ffc60cbac32bc81461f03b895a4a607df9c64b3d1548b',
            'Tangerine-Regular.ttf': '3f5db6010de48f7173939f16621c0e0e794b589eec04e3b04e0a81be848dfab9',
        },
    },
    {
        'id': 'almendra', 'name': 'Almendra', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/almendra/OFL.txt',
        'substitutes': 'Papyrus',
        'files': {
            'Almendra-Bold.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/almendra/Almendra-Bold.ttf',
                ('Almendra', 'Bold')),
            'Almendra-BoldItalic.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/almendra/Almendra-BoldItalic.ttf',
                ('Almendra', 'Bold Italic')),
            'Almendra-Italic.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/almendra/Almendra-Italic.ttf',
                ('Almendra', 'Italic')),
            'Almendra-Regular.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/almendra/Almendra-Regular.ttf',
                ('Almendra', 'Regular')),
        },
        'sha256': {
            'Almendra-Bold.ttf': '5895ef04d0d56c60083ec302e72db99b952b044ab2b440ee12283649b5edc971',
            'Almendra-BoldItalic.ttf': '10e0a7dd4b9f5ad7d51be7f8ee42d76919c322dcbb7733dbad48f4ac751cb105',
            'Almendra-Italic.ttf': '972eb95dd030592755064c35e8e394b75423973d0b7dbfd759c7e36ef80bb7cb',
            'Almendra-Regular.ttf': 'b127a6121209353b53da9ce73bf9d350f74190d8384c28ede179e4fb9440f946',
        },
    },
    {
        'id': 'lobster', 'name': 'Lobster', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/lobster/OFL.txt',
        'substitutes': 'Party LET',
        'files': {
            'Lobster-Regular.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/lobster/Lobster-Regular.ttf',
                ('Lobster', 'Regular')),
        },
        'sha256': {
            'Lobster-Regular.ttf': 'd6568e697fd50cedc0be04d8aae4127fe95add607e7bff954ca88604be80c205',
        },
    },
    {
        'id': 'cutive', 'name': 'Cutive', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/cutive/OFL.txt',
        'substitutes': 'American Typewriter',
        'files': {
            'Cutive-Regular.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/cutive/Cutive-Regular.ttf',
                ('Cutive', 'Regular')),
        },
        'sha256': {
            'Cutive-Regular.ttf': '8f0eb3280328c0051bbe725ce9d68c9117484b769cf68dbacec4343ae92ee031',
        },
    },
    {
        'id': 'notosanssymbols2', 'name': 'Noto Sans Symbols 2', 'licence': 'OFL-1.1',
        'licence_url': 'https://raw.githubusercontent.com/google/fonts/main/ofl/notosanssymbols2/OFL.txt',
        'substitutes': 'Symbol, Zapf Dingbats',
        'files': {
            'NotoSansSymbols2-Regular.ttf': (
                'https://raw.githubusercontent.com/google/fonts/main/ofl/notosanssymbols2/NotoSansSymbols2-Regular.ttf',
                ('Noto Sans Symbols 2', 'Regular')),
        },
        'sha256': {
            'NotoSansSymbols2-Regular.ttf': '7d5fb73b7ca67a6798101741f5d280a3d016a56a197afcd4199dbb57b4b82a21',
        },
    },
]


def fetch(url):
    request = urllib.request.Request(url, headers={'User-Agent': 'taphle-fetch-fonts'})
    with urllib.request.urlopen(request, timeout=120) as response:
        return response.read()


def name_table(data):
    """The (family, subfamily) a font reports, read from its `name` table.

    Hand-rolled because the alternative is a build dependency on fontTools for
    four fields, and because a file this cannot read is one tapHLE's own parser
    is unlikely to accept either.
    """
    if len(data) < 12 or data[:4] == b'ttcf':
        return None
    num_tables = struct.unpack('>H', data[4:6])[0]
    name_offset = None
    for index in range(num_tables):
        entry = 12 + index * 16
        if entry + 16 > len(data):
            return None
        if data[entry:entry + 4] == b'name':
            name_offset = struct.unpack('>I', data[entry + 8:entry + 12])[0]
            break
    if name_offset is None or name_offset + 6 > len(data):
        return None
    count, string_offset = struct.unpack('>HH', data[name_offset + 2:name_offset + 6])
    found = {}
    for index in range(count):
        record = name_offset + 6 + index * 12
        if record + 12 > len(data):
            break
        platform, _encoding, _language, name_id, length, offset = struct.unpack(
            '>HHHHHH', data[record:record + 12])
        start = name_offset + string_offset + offset
        raw = data[start:start + length]
        if not raw:
            continue
        try:
            text = raw.decode('utf-16-be') if platform in (0, 3) else raw.decode('mac-roman')
        except (UnicodeDecodeError, LookupError):
            continue
        found.setdefault(name_id, text)
    # 16/17 are the typographic names, which is what a family with more than
    # four styles uses; 1/2 are the older pair every font has.
    family = found.get(16) or found.get(1)
    subfamily = found.get(17) or found.get(2)
    if family is None or subfamily is None:
        return None
    return family.strip(), subfamily.strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true',
                        help='verify what is already there and download nothing')
    parser.add_argument('--force', action='store_true',
                        help='fetch every file again')
    args = parser.parse_args()

    os.makedirs(FONTS, exist_ok=True)
    os.makedirs(LICENSES, exist_ok=True)

    failures = 0
    unpinned = []
    learned = []
    for family in FAMILIES:
        archive = None
        for filename, (source, expected) in sorted(family['files'].items()):
            path = os.path.join(FONTS, filename)
            have = os.path.exists(path)
            if args.check and not have:
                print('MISSING  %s' % filename)
                failures += 1
                continue
            if have and not args.force:
                data = open(path, 'rb').read()
            elif args.check:
                continue
            elif 'archive' in family:
                if archive is None:
                    print('fetching %s' % family['archive'])
                    try:
                        archive = zipfile.ZipFile(io.BytesIO(fetch(family['archive'])))
                    except Exception as error:
                        print('FAILED   archive for %s: %s' % (family['id'], error))
                        failures += 1
                        break
                try:
                    data = archive.read(source)
                except KeyError:
                    print('FAILED   %s: %s is not in the archive' % (filename, source))
                    failures += 1
                    continue
            else:
                print('fetching %s' % filename)
                try:
                    data = fetch(source)
                except Exception as error:
                    print('FAILED   %s: %s' % (filename, error))
                    failures += 1
                    continue

            digest = hashlib.sha256(data).hexdigest()
            pinned = family.get('sha256', {}).get(filename)
            if pinned and digest != pinned:
                print('HASH     %s: got %s, expected %s' % (filename, digest, pinned))
                failures += 1
                continue
            if not pinned:
                unpinned.append((family['id'], filename, digest))

            names = name_table(data)
            if names is None:
                print('UNPARSED %s: no readable name table' % filename)
                failures += 1
                continue
            if expected is None:
                learned.append((family['id'], filename, names))
            elif names != expected:
                print('WRONG    %s: reports %r, expected %r' % (filename, names, expected))
                failures += 1
                continue

            if not have or args.force:
                with open(path, 'wb') as out:
                    out.write(data)
            print('ok       %-34s %s / %s' % (filename, names[0], names[1]))

        licence_path = os.path.join(LICENSES, 'LICENSE.%s.txt' % family['id'])
        if not os.path.exists(licence_path) and not args.check:
            try:
                with open(licence_path, 'wb') as out:
                    out.write(fetch(family['licence_url']))
                print('ok       %s' % os.path.basename(licence_path))
            except Exception as error:
                print('FAILED   licence for %s: %s' % (family['id'], error))
                failures += 1

    if learned:
        print('\nNames read from the files, to be written into the manifest:')
        for family_id, filename, names in learned:
            print("    %-18s %-34s %r" % (family_id, filename, names))
    if unpinned:
        print('\nHashes to pin, once the names above are right:')
        for family_id, filename, digest in unpinned:
            print("    %-18s %-34s %s" % (family_id, filename, digest))

    if failures:
        print('\n%d problem(s).' % failures)
        return 1
    print('\nAll fonts present and correct.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
