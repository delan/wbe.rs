wbe.rs
======

this project loosely implements the [Web Browser Engineering](https://browser.engineering) book in rust.

## notes

* enter key does not work in location bar yet, so click “go” or press tab-enter or tab-space
* no arrow keys or page up/down keys yet, so use the mouse wheel or similar to scroll
* redirects currently panic! (redirects are common for http → https and trailing slashes)

## getting started

* `nix-shell --run 'cargo fmt && cargo run --release -- https://browser.engineering'`
* build environment vars
    * WBE_FONT_PATH (required) = path to your default font
    * WBE_FONT_PATH_B (required) = path to that font in bold
    * WBE_FONT_PATH_I (required) = path to that font in italic
    * WBE_FONT_PATH_BI (required) = path to that font in bold italic
    * WBE_TIMING_MODE (optional) = if present, quit after first layout
    * WBE_DEBUG_RWLOCK (optional) = if present, print backtraces of RwLock acquisitions
* runtime environment vars
    * RUST_BACKTRACE (optional) = set to 1 or full to print backtraces of panics
    * RUST_LOG (optional) = configure logging in [tracing_subscriber::EnvFilter](https://docs.rs/tracing-subscriber/0.3.16/tracing_subscriber/filter/struct.EnvFilter.html)
        * e.g. RUST_LOG=info,wbe=debug,wbe::layout=trace
    * WINIT_X11_SCALE_FACTOR (optional) = set the ratio of real pixels to css px

## bonus features

* [x] location bar with double buffering
* [x] async load/parse/layout
* [ ] incremental load/parse/layout
* html parser
    * [x] correct handling of [end-tag-with-attributes error](https://html.spec.whatwg.org/#parse-error-end-tag-with-attributes)
    * [x] correct handling of [end-tag-with-trailing-solidus error](https://html.spec.whatwg.org/#parse-error-end-tag-with-trailing-solidus)
    * [x] text inside \<style> is RAWTEXT
    * [x] text inside \<script> is RAWTEXT (technically should be its own unique thing for \<!--)
    * [x] implicit \</p> when opening any tag in (p, table, form, h1, h2, h3, h4, h5, h6)
    * [x] implicit \</li> when opening \<li>
    * [x] implicit \</dt> when opening any tag in (dt, dd)
    * [x] implicit \</dd> when opening any tag in (dt, dd)
    * [x] implicit \</tr> when opening \<tr>
    * [x] implicit \</td>\</tr> when opening \<tr>
    * [x] implicit \</th>\</tr> when opening \<tr>
    * [x] implicit \</td> when opening any tag in (td, th)
    * [x] implicit \</th> when opening any tag in (td, th)
* layout
    * [x] simultaneous wrapping of english and chinese via [uax #29](https://crates.io/crates/unicode-segmentation)

## completion

* [x] chapter 1, downloading web pages
    * [ ] exercise: http/1.1 and user-agent
    * [ ] exercise: file url scheme
    * [x] exercise: data url scheme
    * [ ] ~~exercise: body tag filter~~
    * [x] exercise: entities
    * [ ] exercise: view-source
    * [ ] exercise: compression
    * [ ] exercise: redirects
    * [ ] exercise: caching
* [x] chapter 2, drawing to the screen
    * [ ] ~~exercise: line breaks~~
    * [x] exercise: mouse wheel
    * [ ] exercise: emoji
    * [x] exercise: resizing
    * [ ] exercise: zoom
* [x] chapter 3, formatting text
    * [x] word by word
    * [x] styling text
    * [x] text of different sizes
    * [ ] exercise: centered text
    * [ ] exercise: superscripts
    * [ ] exercise: soft hyphens
    * [ ] exercise: small caps
    * [ ] exercise: preformatted text
* [x] chapter 4, constructing a document tree
    * [ ] handling author errors (implicit \<html/head/body>)
    * [x] exercise: comments
    * [x] exercise: paragraphs
    * [x] exercise: scripts
    * [x] exercise: quoted attributes
    * [ ] exercise: syntax highlighting
* [x] chapter 5, laying out pages
    * [ ] backgrounds
    * [ ] exercise: links bar (hardcoded nav.links style)
    * [x] exercise: hidden head
    * [ ] exercise: bullets
    * [ ] exercise: scrollbar
    * [ ] exercise: table of contents (hardcoded nav#toc style)
    * [ ] exercise: anonymous block boxes
    * [ ] exercise: run-ins (presentational hints for h6)
* [x] chapter 6: applying user styles
    * [x] the style attribute
    * [x] applying style sheets
        * [ ] \<link rel=stylesheet>
    * [ ] cascading
    * [x] inherited styles
    * [ ] exercise: fonts
    * [x] exercise: width
    * [x] exercise: height
    * [x] exercise: class selectors
    * [x] exercise: display
    * [x] exercise: shorthand properties
    * [x] exercise: fast descendant selectors
    * [x] exercise: selector sequences (compound selectors)
    * [ ] exercise: important
    * [ ] exercise: ancestor selectors
    * [x] exercise: inline style sheets
