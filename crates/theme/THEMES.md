# Bundled themes

All files in `themes/` use Zed's theme-family JSON format and are loaded by
`optchat_theme::builtin()`. Third-party files are bundled **unmodified**.

| File | Themes | Origin | License |
|---|---|---|---|
| `optchat.json` | OptChat Dark, OptChat Light | Authored for OptChat (Dark mirrors the OptChat web palette). Carries an `"optchat"` extension object inside `style`. | Same as this project |
| `one.json` | One Dark, One Light | [zed-industries/zed](https://github.com/zed-industries/zed) `assets/themes/one/one.json` (main, fetched 2026-10-10); based on Atom One ([atom/atom](https://github.com/atom/atom)) | MIT, Copyright (c) 2014 GitHub Inc. |
| `ayu.json` | Ayu Dark, Ayu Light, Ayu Mirage | zed `assets/themes/ayu/ayu.json` (main, fetched 2026-10-10); based on [dempfi/ayu](https://github.com/dempfi/ayu) | MIT, Copyright (c) 2016 Ike Ku |
| `gruvbox.json` | Gruvbox Dark / Dark Hard / Dark Soft / Light / Light Hard / Light Soft | zed `assets/themes/gruvbox/gruvbox.json` (main, fetched 2026-10-10); based on [morhetz/gruvbox](https://github.com/morhetz/gruvbox) by Pavel Pertsev | MIT (Zed's license file carries the template line "Copyright (c) <YEAR> <COPYRIGHT HOLDER>"; reproduced as-is below) |
| `solarized.json` | Solarized Dark, Solarized Light | zed `assets/themes/solarized/solarized.json` at commit [`da1c3d8b402d`](https://github.com/zed-industries/zed/tree/da1c3d8b402d38d1fe9e5d201a2b38de6187b092/assets/themes/solarized) (removed from Zed in 58b0a6c4, "Remove legacy themes and licenses", 2025-02-14); based on [altercation/solarized](https://github.com/altercation/solarized) | MIT, Copyright (c) 2011 Ethan Schoonover |
| `tomorrow.json` | Tomorrow Night, Tomorrow Night Blue | Zed never shipped these, so they were authored for OptChat in Zed format from the canonical palettes of [chriskempson/tomorrow-theme](https://github.com/chriskempson/tomorrow-theme) (editor colors are the canonical values; UI chrome shades derived from them) | Palette: MIT, Copyright (C) 2011 Chris Kempson |

Zed's own theme JSON packaging is from the Zed editor repository
(Copyright Zed Industries, Inc.); the per-theme licenses Zed ships with these
files (`assets/themes/LICENSES` and the per-directory `LICENSE` files) are
reproduced below.

## License texts

### One Dark / One Light

The MIT License (MIT)

Copyright (c) 2014 GitHub Inc.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

### Ayu Dark / Ayu Light / Ayu Mirage

The MIT License (MIT)

Copyright (c) 2016 Ike Ku

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

### Gruvbox (all variants)

The MIT License (MIT)

Copyright (c) <YEAR> <COPYRIGHT HOLDER>

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

### Solarized Dark / Solarized Light

The MIT License (MIT)

Copyright (c) 2011 Ethan Schoonover

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

### Tomorrow Night / Tomorrow Night Blue (palette)

The MIT License (MIT)

Copyright (C) 2011 Chris Kempson

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
