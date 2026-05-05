# Lora — font source and license

## Source

Downloaded 2026-05-05 from the Google Fonts CDN via the `css2` API:

    https://fonts.googleapis.com/css2?family=Lora:ital,wght@0,400;0,700;1,400;1,700&display=swap

The CSS response (with a desktop Chrome `User-Agent`) advertises woff2 files per
unicode-range. We took the `latin` (U+0000-00FF) subset for each style/weight.

Underlying files (Google Fonts v37):

| Local filename          | Source URL                                                        |
| ----------------------- | ----------------------------------------------------------------- |
| `Lora-Regular.woff2`    | `https://fonts.gstatic.com/s/lora/v37/0QIvMX1D_JOuMwr7I_FMl_E.woff2`        |
| `Lora-Bold.woff2`       | `https://fonts.gstatic.com/s/lora/v37/0QIvMX1D_JOuMwr7I_FMl_E.woff2`        |
| `Lora-Italic.woff2`     | `https://fonts.gstatic.com/s/lora/v37/0QIhMX1D_JOuMw_LIftLtfOm8w.woff2`     |
| `Lora-BoldItalic.woff2` | `https://fonts.gstatic.com/s/lora/v37/0QIhMX1D_JOuMw_LIftLtfOm8w.woff2`     |

Note: Google Fonts ships Lora as a variable font with a weight axis, so the
upright 400 / 700 pair share one URL and the italic 400 / 700 pair share
another. Browsers select the right weight via the `font-weight` declarations in
each `@font-face` rule in `static/style.css`. The four file names above are
kept distinct so the CSS stays explicit and so a future swap to four genuinely
distinct static files is a drop-in replacement.

Upstream project (canonical source, identical license):
<https://github.com/cyrealtype/Lora-Cyrillic>

## License

Lora is licensed under the **SIL Open Font License, Version 1.1** (OFL-1.1).
Full license text: <https://openfontlicense.org/open-font-license-official-text/>

Permissions used here:
- Embed the font in a web product (this repo, served from `/static/fonts/`).
- Redistribute the font file alongside the product.

Obligations honoured:
- The font is not sold by itself.
- Reserved Font Names are not used in derivative works (we do not modify the
  files; we ship them as-downloaded).
- This SOURCE.md preserves the OFL notice.

## Refresh procedure

To re-pull from Google Fonts:

```sh
cd static/fonts
UA='Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
    (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
curl -sS -A "$UA" \
  "https://fonts.googleapis.com/css2?family=Lora:ital,wght@0,400;0,700;1,400;1,700&display=swap" \
  | grep -E "src: url\(|font-style|font-weight|U\+0000-00FF"
# Then curl the `latin` URL for each style/weight into the matching filename.
```
