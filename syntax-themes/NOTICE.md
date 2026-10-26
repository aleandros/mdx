# Vendored Syntax Themes

This directory contains syntax highlighting themes vendored from third-party
sources and embedded into the mdx binary with `include_bytes!`.

## Nord.tmTheme

- Source: https://github.com/nordtheme/sublime-text (formerly
  `arcticicestudio/nord-sublime-text`)
- Commit: `56b3d7a126eb92459ec47ec843e29eb6dad6480f` (2019-05-11), the last
  revision that shipped `Nord.tmTheme` before the project moved to the
  `.sublime-color-scheme` format
- Vendored: 2026-10-26
- License: MIT (reproduced below)
- Modifications: every `#616e88` foreground (comments, `markup.quote`, GitGutter
  ignored) is lightened to `#7b88a1`, raising its contrast against the nord0
  background `#2E3440` from 2.43:1 to 3.50:1. The change is also noted in the
  file's header comment.

```
MIT License (MIT)

Copyright (c) 2017-present Arctic Ice Studio <development@arcticicestudio.com> (http://arcticicestudio.com)
Copyright (c) 2017-present Sven Greb <code@svengreb.de> (http://svengreb.de)

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
```
