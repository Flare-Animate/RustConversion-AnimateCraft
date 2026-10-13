# Contributing to AnimationCraft

Thanks for helping. AnimationCraft is an independent, open-source animation
tool that offers a workflow familiar to users of Adobe Animate (formerly Flash
Professional) and can exchange files with it. To keep the project safe to use,
fork and ship, **every contribution must be clean-room**. Please read this
whole file before your first pull request.

*This is a project policy, not legal advice. When in doubt, ask in an issue
before you write the code.*

## 1. Clean-room rules

AnimationCraft is written from **public information and observed behaviour**,
never from Adobe's code or the inside of Adobe's programs.

### Allowed sources

- Public documentation: user guides, help pages, tutorials, the published JSFL
  API reference, release notes, books, videos and blog posts.
- **Watching how a program behaves** through its normal user interface: what
  a tool does when you click and drag, what a menu item changes, how a tween
  looks frame by frame.
- **Files you authored yourself.** XFL is plain XML, so you may open and read
  `.fla`/XFL documents *you* created (or that their author gave you permission
  to use) to understand the file format, purely for interoperability.
- Public, openly licensed specifications and libraries (SVG, PNG, ZIP, XMP,
  Lottie, CreateJS docs, Ruffle's MIT/Apache crates, and so on).
- Common knowledge and standard algorithms (Bezier maths, Penner easing
  equations, polygon booleans, linear-blend skinning, …).

### Not allowed

- **Decompiling, disassembling, debugging, patching or memory-inspecting**
  Adobe Animate or any other Adobe program, plug-in or runtime, or using
  output from someone who did. Adobe's licence terms restrict reverse
  engineering of its software, and we don't rely on legal exceptions for it.
- Copying **code** from Adobe products, SDKs or samples, including JSFL
  scripts, CreateJS templates shipped *inside* Animate, or ActionScript
  sources.
- Copying **artwork or resources** out of an install: icons, cursors, splash
  screens, brushes, fonts, colour swatches, motion/ease preset files, sample
  documents or templates. Draw or write your own.
- Copying **documentation text**. Describe behaviour in your own words; short
  quotations of command names and attribute names are fine.
- Using Adobe **sample files** or other people's `.fla` files without the
  author's permission, in tests or anywhere else.
- Information obtained under an NDA, a beta agreement or an employment
  contract with Adobe.

If you have worked on Animate's source code at Adobe, please tell the
maintainers before contributing to the related areas.

### Interoperability identifiers

Reading and writing `.fla`/XFL requires a few **functional names**: element
and attribute names (`DOMDocument`, `DOMFrame`, `fillStyle0`, …), the XFL XML
namespace `http://ns.adobe.com/xfl/2008/`, and the package mimetype
`application/vnd.adobe.xfl`. They appear **only** in the XFL import/export
code (`crates/ac-xfl`), its hand-written test fixture and the `.fla`/XFL files
it writes, and only so other tools recognise those files. Don't use them
anywhere else, and never in AnimationCraft's own `.acx` format.

## 2. Names and trademarks

- "Adobe", "Animate", "Flash" and their logos are Adobe's trademarks. Use
  them only **to describe** compatibility or history ("opens .fla files",
  "familiar to Adobe Animate users", "Flash's merge drawing model"), never in
  the product name, app icons, splash screens, package names or marketing
  that could suggest endorsement. Keep the "Not affiliated with Adobe" note in
  the About box and README.
- Don't imitate Adobe's visual branding (colours, icon style, logo shapes).
- The working name "AnimationCraft" follows the ArtCraft family naming; see
  `CLAUDE.md` before any public release.

## 3. Test files and media

- Test documents must be **made by you or by AnimationCraft**, or shared by
  their author with explicit permission (say so in the pull request and keep
  a note next to the file). Hand-written XFL fixtures live in
  `crates/ac-xfl/tests/fixtures/` with a comment saying they are hand-written.
- Demo content, screenshots and sample art must be original or public domain.
- Never commit Adobe sample files, templates or presets.

## 4. Third-party code

- Dependencies must use a licence compatible with our `MIT OR Apache-2.0`
  licence: MIT, Apache-2.0, BSD, ISC, Zlib, Unicode, CC0, OFL/UFL (fonts) or
  MPL-2.0 (used unmodified, e.g. Symphonia, usvg). **No GPL/AGPL/LGPL-only**
  dependencies in the default build.
- Code copied from another open-source project keeps its licence header and
  is listed in the pull request.

## 5. AI-assisted contributions

AI tools are welcome, and much of this codebase was written with one. The
same rules apply to their output: don't prompt a tool to reproduce Adobe code
or documentation, don't paste decompiled material into it, and review what it
produces as if you wrote it. **You** are responsible for the contribution.

## 6. Developer Certificate of Origin

Every commit must be signed off (`git commit -s`), which adds:

    Signed-off-by: Your Name <you@example.com>

By signing off you certify the [Developer Certificate of Origin
1.1](https://developercertificate.org/): you wrote the change (or have the
right to submit it) under the project's licence, **and it follows the
clean-room rules above**.

## 7. Workflow

1. Read `CLAUDE.md` (project brief) and `docs/ANIMATE-GAP-ANALYSIS.md` (what
   is planned and in what order).
2. Keep changes focused; new behaviour gets a test.
3. Before opening a pull request:
   - `cargo test --workspace`
   - `cargo clippy --workspace --all-targets` (no warnings)
   - for UI changes, look at the result (`AC_SCREENSHOT=out.png cargo run -p ac-app` works under Xvfb)
   - add a short entry to `log/devlog.md`: what changed and **where the knowledge came from** when it
     concerns compatibility (e.g. "attribute X observed in an XFL file I saved").
4. Shortcuts follow Animate's defaults; menus and panels may be organised
   similarly, but use our own wording and artwork.

## 8. Reporting a concern

If you think something in the repository breaks these rules, open an issue
(or contact the maintainers privately) with the file and the reason. We will
look at it promptly and remove or rewrite anything that doesn't belong.
