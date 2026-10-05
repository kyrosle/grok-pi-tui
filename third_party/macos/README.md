# macOS binary library notices

The macOS package includes non-system dylibs only when the build links them.
These libraries remain dynamically linked and replaceable under
`lib/grok-pi/`; the packaging step changes install names and applies ad-hoc
signatures. It does not change their source code.

- GNU libiconv **1.17**, `libiconv.2.dylib`: LGPL-2.1-or-later.
  License: [COPYING.libiconv](COPYING.libiconv).
  Corresponding source: <https://ftp.gnu.org/pub/gnu/libiconv/libiconv-1.17.tar.gz>.
- zlib **1.3.1**, `libz.1.dylib`: zlib license.
  License: [LICENSE.zlib](LICENSE.zlib).
  Corresponding source: <https://zlib.net/fossils/zlib-1.3.1.tar.gz>.

The bundled dylibs require macOS 14 or newer.

The v0.1.10 GitHub Release also hosts those exact source archives. These
notices and licenses are included in the binary archive and installed with
the libraries. System frameworks and `/usr/lib` libraries are not bundled.
