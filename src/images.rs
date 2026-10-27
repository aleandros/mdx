//! Local images for the pager: path resolution, background decoding, and the
//! Kitty transmissions and placements each image needs.
//!
//! The pager owns one [`ImageStore`] when graphics are on. Decoding runs on a
//! worker thread, so a large image never blocks scrolling; until it is ready
//! (and whenever it fails) the pager keeps the text placeholder. Escape
//! sequences are queued here and written by the event loop before a frame.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::SystemTime;

use crate::kitty;

/// Longest side, in pixels, of the data sent to the terminal. Larger images
/// are scaled down first; the terminal scales them to the placement.
const MAX_TRANSMIT_PX: u32 = 2048;

/// Name of the decoding thread. The panic hook leaves the terminal alone for
/// it: a decoder panic only fails that image.
pub(crate) const WORKER_THREAD: &str = "mdx-images";

/// Whether images may be drawn under the `NO_COLOR` value `no_color`.
/// Crossterm drops all colors when it is set and not empty, and the image id
/// travels in the foreground color, so placeholders would stay blank.
pub(crate) fn allowed_with_no_color(no_color: Option<&std::ffi::OsStr>) -> bool {
    no_color.is_none_or(std::ffi::OsStr::is_empty)
}

/// The local file an image URL refers to: an absolute path, a path relative
/// to `base_dir` (the markdown file's directory), or a `file://` URL. Remote
/// and other URLs return `None`.
pub(crate) fn resolve(url: &str, base_dir: &Path) -> Option<PathBuf> {
    let url = url.trim();
    let path = if let Some(rest) = url.strip_prefix("file://") {
        rest
    } else if has_scheme(url) {
        return None;
    } else {
        url
    };
    if path.is_empty() {
        return None;
    }
    let join = |path: &str| {
        let path = Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            base_dir.join(path)
        }
    };
    let resolved = join(path);
    // Markdown often escapes spaces (`my%20pic.png`); use the decoded name
    // when only that one exists.
    if !resolved.exists()
        && let Some(decoded) = percent_decode(path)
    {
        let decoded = join(&decoded);
        if decoded.exists() {
            return Some(decoded);
        }
    }
    Some(resolved)
}

/// `%XX` escapes decoded, or `None` when there are none or they are invalid.
fn percent_decode(text: &str) -> Option<String> {
    if !text.contains('%') {
        return None;
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// `scheme:` with a scheme of two or more characters, so `C:\x` stays a path.
fn has_scheme(url: &str) -> bool {
    url.split_once(':').is_some_and(|(scheme, _)| {
        scheme.len() > 1
            && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

/// A decoded image, re-encoded as PNG for transmission.
#[derive(Debug)]
struct Decoded {
    /// Size of the original image, which sets the aspect ratio and the
    /// natural size.
    px: (u32, u32),
    png: Vec<u8>,
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Decode a PNG, JPEG or GIF (first frame) and re-encode it as RGBA PNG,
/// scaled down to [`MAX_TRANSMIT_PX`] when larger.
fn decode(path: &Path) -> Result<Decoded, image::ImageError> {
    // Only regular files: a FIFO would block the worker, and a device such
    // as /dev/tty would read the user's keystrokes.
    if !std::fs::metadata(path)?.is_file() {
        return Err(image::ImageError::IoError(std::io::Error::other(
            "not a regular file",
        )));
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(256 * 1024 * 1024);
    let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
    reader.limits(limits);
    let img = reader.decode()?;
    let px = (img.width(), img.height());
    let img = if px.0 > MAX_TRANSMIT_PX || px.1 > MAX_TRANSMIT_PX {
        img.resize(
            MAX_TRANSMIT_PX,
            MAX_TRANSMIT_PX,
            image::imageops::FilterType::Triangle,
        )
    } else {
        img
    };
    let img = image::DynamicImage::ImageRgba8(img.to_rgba8());
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)?;
    Ok(Decoded { px, png })
}

impl From<crate::mermaid::svg::Raster> for Decoded {
    fn from(raster: crate::mermaid::svg::Raster) -> Self {
        Decoded {
            px: raster.px,
            png: raster.png,
        }
    }
}

/// A rasterized diagram or formula: the source key and cell size it was
/// drawn for.
type DiagramKey = (u64, (u16, u16));

#[derive(Debug)]
enum Job {
    Decode(PathBuf),
    Rasterize(DiagramKey, String),
    /// Typeset display math for the key's cell size, then rasterize it.
    Typeset(DiagramKey, Arc<crate::math::MathImage>),
}

#[derive(Debug)]
enum Done {
    /// A finished decode: the path, the file's mtime before decoding, and
    /// the image (or `None` if the file is missing or not a supported image).
    Decoded(PathBuf, Option<SystemTime>, Option<Decoded>),
    /// A finished diagram or formula, or `None` if it could not be drawn.
    Rasterized(DiagramKey, Option<Decoded>),
}

/// The decoding thread. Dropping it closes the request channel, which ends
/// the thread after its current image.
#[derive(Debug)]
struct Worker {
    requests: Sender<Job>,
    results: Receiver<Done>,
}

impl Worker {
    fn spawn() -> Option<Self> {
        let (requests, request_rx) = mpsc::channel::<Job>();
        let (result_tx, results) = mpsc::channel();
        std::thread::Builder::new()
            .name(WORKER_THREAD.into())
            .spawn(move || {
                for job in request_rx {
                    // A decoder or rasterizer panic fails this image, not
                    // the pager.
                    let done = match job {
                        Job::Decode(path) => {
                            let mtime = modified(&path);
                            let decoded = std::panic::catch_unwind(|| decode(&path))
                                .ok()
                                .and_then(Result::ok);
                            Done::Decoded(path, mtime, decoded)
                        }
                        Job::Rasterize(key, svg) => {
                            let raster =
                                std::panic::catch_unwind(|| crate::mermaid::svg::rasterize(&svg))
                                    .ok()
                                    .and_then(Result::ok);
                            Done::Rasterized(key, raster.map(Decoded::from))
                        }
                        Job::Typeset(key, math) => {
                            let raster = std::panic::catch_unwind(|| math.rasterize(key.1))
                                .ok()
                                .and_then(Result::ok);
                            Done::Rasterized(key, raster.map(Decoded::from))
                        }
                    };
                    if result_tx.send(done).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        Some(Worker { requests, results })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    /// Decoding for the first time.
    Pending,
    /// Transmitted (or queued for it); `px` is the original size.
    Ready { px: (u32, u32) },
    /// Missing, unreadable or unsupported: the text placeholder stays.
    Failed,
}

#[derive(Debug)]
struct Entry {
    id: u32,
    status: Status,
    /// Modification time of the file the current image was decoded from.
    mtime: Option<SystemTime>,
    /// Placement size last sent to the terminal.
    placed: Option<(u16, u16)>,
    /// A decode request is in flight.
    loading: bool,
}

/// A diagram or formula drawn as an image.
#[derive(Debug)]
struct DiagramEntry {
    id: u32,
    /// `Ready` holds the raster size. A diagram is placed over its own cell
    /// rectangle; a formula over the cells its raster covers.
    status: Status,
    placed: Option<(u16, u16)>,
}

/// Images shown in the pager: local files keyed by resolved path, and
/// rasterized diagrams and formulas keyed by source and cell size.
#[derive(Debug)]
pub(crate) struct ImageStore {
    base_dir: PathBuf,
    cell_px: (u16, u16),
    entries: HashMap<PathBuf, Entry>,
    diagrams: HashMap<DiagramKey, DiagramEntry>,
    worker: Option<Worker>,
    /// Start the worker on the first request; off in tests.
    spawn_worker: bool,
    next_id: u32,
    /// Escape sequences waiting to be written to the terminal.
    output: Vec<u8>,
    /// Ids whose data reached the terminal and must be deleted on exit.
    sent: HashSet<u32>,
    /// Wrap output in tmux passthrough, for a pager running inside tmux.
    via_tmux: bool,
}

impl ImageStore {
    /// A store that decodes on a background thread, started when the first
    /// image is requested.
    pub(crate) fn new(base_dir: PathBuf, cell_px: (u16, u16)) -> Self {
        let mut store = Self::without_worker(base_dir, cell_px);
        store.spawn_worker = true;
        store
    }

    fn without_worker(base_dir: PathBuf, cell_px: (u16, u16)) -> Self {
        ImageStore {
            base_dir,
            cell_px,
            entries: HashMap::new(),
            diagrams: HashMap::new(),
            worker: None,
            spawn_worker: false,
            next_id: first_id(),
            output: Vec::new(),
            sent: HashSet::new(),
            via_tmux: false,
        }
    }

    /// Wrap every escape sequence in tmux passthrough, so the terminal
    /// outside tmux receives it.
    pub(crate) fn through_tmux(mut self) -> Self {
        self.via_tmux = true;
        self
    }

    /// `bytes` as written to the terminal: wrapped for tmux when needed.
    fn encode(&self, bytes: Vec<u8>) -> Vec<u8> {
        if self.via_tmux {
            kitty::tmux_passthrough(&bytes)
        } else {
            bytes
        }
    }

    /// Record a new cell size. Diagrams drawn for the old size are deleted;
    /// they are rasterized again for the new one when next laid out.
    pub(crate) fn set_cell_px(&mut self, cell_px: (u16, u16)) {
        if cell_px == self.cell_px {
            return;
        }
        self.cell_px = cell_px;
        let output = &mut self.output;
        self.diagrams.retain(|(_, px), entry| {
            let kept = *px == cell_px;
            if !kept && matches!(entry.status, Status::Ready { .. }) {
                output.extend(kitty::delete(entry.id));
            }
            kept
        });
    }

    /// Whether a decode or rasterization is in flight, so the event loop
    /// should poll.
    pub(crate) fn loading(&self) -> bool {
        self.entries.values().any(|e| e.loading)
            || self.diagrams.values().any(|d| d.status == Status::Pending)
    }

    fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        // Ids stay within 24 bits (no high-byte diacritic) and skip 0.
        self.next_id = (self.next_id % 0x00ff_ffff) + 1;
        id
    }

    fn send(&mut self, job: Job) -> bool {
        if self.worker.is_none() && self.spawn_worker {
            self.spawn_worker = false;
            self.worker = Worker::spawn();
        }
        self.worker
            .as_ref()
            .is_some_and(|w| w.requests.send(job).is_ok())
    }

    fn request(&mut self, path: &Path) -> bool {
        self.send(Job::Decode(path.to_path_buf()))
    }

    /// Image id and placement for `diagram`, drawn as an image over exactly
    /// its ASCII cell rectangle, or `None` while it rasterizes, when it
    /// failed, or when it is too large to address. The first call for a
    /// diagram and cell size starts rasterizing it.
    pub(crate) fn diagram(
        &mut self,
        diagram: &crate::mermaid::svg::DiagramImage,
    ) -> Option<ImageLayout> {
        let (cols, rows) = diagram.cells();
        let cols = u16::try_from(cols)
            .ok()
            .filter(|c| (1..=kitty::MAX_CELLS).contains(c))?;
        let rows = u16::try_from(rows)
            .ok()
            .filter(|r| (1..=kitty::MAX_CELLS).contains(r))?;
        let key = (diagram.key(), self.cell_px);
        if !self.diagrams.contains_key(&key) {
            let id = self.alloc_id();
            let svg = diagram.to_svg(self.cell_px);
            let status = if self.send(Job::Rasterize(key, svg)) {
                Status::Pending
            } else {
                Status::Failed
            };
            let entry = DiagramEntry {
                id,
                status,
                placed: None,
            };
            self.diagrams.insert(key, entry);
            return None;
        }
        let entry = self.diagrams.get_mut(&key)?;
        if !matches!(entry.status, Status::Ready { .. }) {
            return None;
        }
        if entry.placed != Some((cols, rows)) {
            entry.placed = Some((cols, rows));
            self.output.extend(kitty::place(entry.id, cols, rows));
        }
        Some(ImageLayout {
            id: entry.id,
            cols,
            rows,
        })
    }

    /// Whether `diagram` is still being rasterized for the current cell
    /// size.
    pub(crate) fn diagram_pending(&self, diagram: &crate::mermaid::svg::DiagramImage) -> bool {
        self.diagrams
            .get(&(diagram.key(), self.cell_px))
            .is_some_and(|d| d.status == Status::Pending)
    }

    /// Image id and placement for display math `math`, at its natural size
    /// in cells or scaled down to `max_cols`, or `None` while it typesets,
    /// when it failed, and when it would have to shrink below half its size
    /// (the source text reads better). The first call for a formula and
    /// cell size starts typesetting it.
    pub(crate) fn math(
        &mut self,
        math: &Arc<crate::math::MathImage>,
        max_cols: u16,
    ) -> Option<ImageLayout> {
        let key = (math.key(), self.cell_px);
        if !self.diagrams.contains_key(&key) {
            let id = self.alloc_id();
            let status = if self.send(Job::Typeset(key, Arc::clone(math))) {
                Status::Pending
            } else {
                Status::Failed
            };
            let entry = DiagramEntry {
                id,
                status,
                placed: None,
            };
            self.diagrams.insert(key, entry);
            return None;
        }
        let entry = self.diagrams.get_mut(&key)?;
        let Status::Ready { px } = entry.status else {
            return None;
        };
        let cell_w = if self.cell_px.0 == 0 {
            kitty::FALLBACK_CELL_PX.0
        } else {
            self.cell_px.0
        };
        if px.0.div_ceil(u32::from(cell_w)) > u32::from(max_cols) * 2 {
            return None;
        }
        let (cols, rows) = kitty::fit_cells(px, self.cell_px, max_cols, kitty::MAX_CELLS)?;
        if entry.placed != Some((cols, rows)) {
            entry.placed = Some((cols, rows));
            self.output.extend(kitty::place(entry.id, cols, rows));
        }
        Some(ImageLayout {
            id: entry.id,
            cols,
            rows,
        })
    }

    /// Image id and placement size for `url`, fitted to `max_cols x
    /// max_rows`, or `None` while it loads, when it is not a local image, or
    /// when it failed. The first call for a path starts decoding it. Queues
    /// a placement whenever the size changes.
    pub(crate) fn layout(
        &mut self,
        url: &str,
        max_cols: u16,
        max_rows: u16,
    ) -> Option<ImageLayout> {
        let path = resolve(url, &self.base_dir)?;
        if !self.entries.contains_key(&path) {
            let id = self.alloc_id();
            let loading = self.request(&path);
            let status = if loading {
                Status::Pending
            } else {
                Status::Failed
            };
            let entry = Entry {
                id,
                status,
                mtime: None,
                placed: None,
                loading,
            };
            self.entries.insert(path, entry);
            return None;
        }
        let entry = self.entries.get_mut(&path)?;
        let Status::Ready { px } = entry.status else {
            return None;
        };
        let (cols, rows) = kitty::fit_cells(px, self.cell_px, max_cols, max_rows)?;
        if entry.placed != Some((cols, rows)) {
            entry.placed = Some((cols, rows));
            self.output.extend(kitty::place(entry.id, cols, rows));
        }
        Some(ImageLayout {
            id: entry.id,
            cols,
            rows,
        })
    }

    /// Take finished decodes. A new or changed image is queued for
    /// transmission under its id. Returns whether the layout must be rebuilt.
    pub(crate) fn poll(&mut self) -> bool {
        let Some(worker) = &self.worker else {
            return false;
        };
        let mut changed = false;
        while let Ok(done) = worker.results.try_recv() {
            let (path, mtime, decoded) = match done {
                Done::Decoded(path, mtime, decoded) => (path, mtime, decoded),
                Done::Rasterized(key, raster) => {
                    let Some(entry) = self.diagrams.get_mut(&key) else {
                        continue;
                    };
                    if let Some(raster) = raster {
                        self.output.extend(kitty::transmit(entry.id, &raster.png));
                        entry.status = Status::Ready { px: raster.px };
                    } else {
                        entry.status = Status::Failed;
                    }
                    entry.placed = None;
                    changed = true;
                    continue;
                }
            };
            let Some(entry) = self.entries.get_mut(&path) else {
                continue;
            };
            entry.loading = false;
            entry.mtime = mtime;
            if let Some(decoded) = decoded {
                self.output.extend(kitty::transmit(entry.id, &decoded.png));
                // New data drops the old placement; place it again.
                entry.placed = None;
                entry.status = Status::Ready { px: decoded.px };
            } else {
                // A reload that fails removes the image shown so far.
                if matches!(entry.status, Status::Ready { .. }) {
                    self.output.extend(kitty::delete(entry.id));
                }
                entry.status = Status::Failed;
                entry.placed = None;
            }
            changed = true;
        }
        changed
    }

    /// Decode again every image whose file's mtime changed (watch mode).
    /// The current image stays on screen until the new one arrives.
    pub(crate) fn refresh(&mut self) {
        let stale: Vec<PathBuf> = self
            .entries
            .iter()
            .filter(|(path, e)| !e.loading && modified(path) != e.mtime)
            .map(|(path, _)| path.clone())
            .collect();
        for path in stale {
            let loading = self.request(&path);
            if let Some(entry) = self.entries.get_mut(&path) {
                entry.loading = loading;
            }
        }
    }

    /// Forget the images no URL in `urls` refers to, deleting them from the
    /// terminal.
    pub(crate) fn retain<'a>(&mut self, urls: impl IntoIterator<Item = &'a str>) {
        let keep: HashSet<PathBuf> = urls
            .into_iter()
            .filter_map(|url| resolve(url, &self.base_dir))
            .collect();
        let output = &mut self.output;
        self.entries.retain(|path, entry| {
            let kept = keep.contains(path);
            if !kept && matches!(entry.status, Status::Ready { .. }) {
                output.extend(kitty::delete(entry.id));
            }
            kept
        });
    }

    /// Forget the diagrams and formulas whose key is not in `keys`, or that were drawn
    /// for another cell size, deleting them from the terminal.
    pub(crate) fn retain_diagrams(&mut self, keys: impl IntoIterator<Item = u64>) {
        let keep: HashSet<u64> = keys.into_iter().collect();
        let cell_px = self.cell_px;
        let output = &mut self.output;
        self.diagrams.retain(|(key, px), entry| {
            let kept = *px == cell_px && keep.contains(key);
            if !kept && matches!(entry.status, Status::Ready { .. }) {
                output.extend(kitty::delete(entry.id));
            }
            kept
        });
    }

    /// Escape sequences to write to the terminal before the next frame.
    pub(crate) fn take_output(&mut self) -> Vec<u8> {
        let ready = self
            .entries
            .values()
            .map(|e| (e.id, e.status))
            .chain(self.diagrams.values().map(|d| (d.id, d.status)));
        for (id, status) in ready {
            if matches!(status, Status::Ready { .. }) {
                self.sent.insert(id);
            }
        }
        let output = std::mem::take(&mut self.output);
        self.encode(output)
    }

    /// Delete every image this store sent, so none stays in the terminal's
    /// memory after exit.
    pub(crate) fn delete_all(&mut self) -> Vec<u8> {
        let mut ids: Vec<u32> = self.sent.drain().collect();
        ids.sort_unstable();
        let output = ids.into_iter().flat_map(kitty::delete).collect();
        self.encode(output)
    }

    #[cfg(test)]
    pub(crate) fn for_tests(base_dir: PathBuf, cell_px: (u16, u16)) -> Self {
        Self::without_worker(base_dir, cell_px)
    }

    /// Whether no diagram was ever requested.
    #[cfg(test)]
    pub(crate) fn diagrams_is_empty(&self) -> bool {
        self.diagrams.is_empty()
    }

    /// Mark `diagram` as rasterized, as if the worker had finished.
    #[cfg(test)]
    pub(crate) fn insert_ready_diagram(
        &mut self,
        diagram: &crate::mermaid::svg::DiagramImage,
    ) -> u32 {
        let id = self.alloc_id();
        self.diagrams.insert(
            (diagram.key(), self.cell_px),
            DiagramEntry {
                id,
                status: Status::Ready { px: (1, 1) },
                placed: None,
            },
        );
        id
    }

    /// Mark display math `math` as typeset with size `px`, as if the worker
    /// had finished.
    #[cfg(test)]
    pub(crate) fn insert_ready_math(
        &mut self,
        math: &crate::math::MathImage,
        px: (u32, u32),
    ) -> u32 {
        let id = self.alloc_id();
        self.diagrams.insert(
            (math.key(), self.cell_px),
            DiagramEntry {
                id,
                status: Status::Ready { px },
                placed: None,
            },
        );
        id
    }

    /// Mark `url` as decoded with size `px`, as if the worker had finished.
    #[cfg(test)]
    pub(crate) fn insert_ready(&mut self, url: &str, px: (u32, u32)) -> u32 {
        let path = resolve(url, &self.base_dir).expect("test image URL is local");
        let id = self.alloc_id();
        self.entries.insert(
            path,
            Entry {
                id,
                status: Status::Ready { px },
                mtime: None,
                placed: None,
                loading: false,
            },
        );
        id
    }
}

/// Where an image goes: its id and placement size in cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ImageLayout {
    pub(crate) id: u32,
    pub(crate) cols: u16,
    pub(crate) rows: u16,
}

/// First image id: derived from the process id, so two mdx processes in the
/// same terminal are unlikely to overwrite each other's images.
fn first_id() -> u32 {
    let mixed = std::process::id().wrapping_mul(0x9e37_79b1) >> 8;
    (mixed % 0x00ff_ffff) + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn resolve_local_and_remote() {
        let base = Path::new("/docs");
        assert_eq!(resolve("a.png", base), Some(PathBuf::from("/docs/a.png")));
        assert_eq!(
            resolve("img/a.png", base),
            Some(PathBuf::from("/docs/img/a.png"))
        );
        assert_eq!(
            resolve("/abs/a.png", base),
            Some(PathBuf::from("/abs/a.png"))
        );
        assert_eq!(
            resolve("file:///abs/a.png", base),
            Some(PathBuf::from("/abs/a.png"))
        );
        assert_eq!(resolve("https://example.com/a.png", base), None);
        assert_eq!(resolve("http://example.com/a.png", base), None);
        assert_eq!(resolve("data:image/png;base64,AAAA", base), None);
        assert_eq!(resolve("", base), None);
    }

    #[test]
    fn resolve_decodes_percent_escapes_only_when_needed() {
        let dir = temp_dir("percent");
        std::fs::write(dir.join("my pic.png"), "").unwrap();
        std::fs::write(dir.join("100%.png"), "").unwrap();
        assert_eq!(resolve("my%20pic.png", &dir), Some(dir.join("my pic.png")));
        assert_eq!(resolve("100%.png", &dir), Some(dir.join("100%.png")));
        assert_eq!(
            resolve("no%20such.png", &dir),
            Some(dir.join("no%20such.png"))
        );
        assert_eq!(percent_decode("a%2Fb"), Some("a/b".into()));
        assert_eq!(percent_decode("a%zz"), None);
        assert_eq!(percent_decode("plain"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_color_turns_images_off() {
        use std::ffi::OsStr;
        assert!(allowed_with_no_color(None));
        assert!(allowed_with_no_color(Some(OsStr::new(""))));
        assert!(!allowed_with_no_color(Some(OsStr::new("1"))));
    }

    #[test]
    fn non_regular_files_are_not_read() {
        let dir = temp_dir("special");
        assert!(decode(&dir).is_err(), "a directory is not decoded");
        #[cfg(unix)]
        assert!(decode(Path::new("/dev/null")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn worker_starts_with_the_first_request() {
        let mut store = ImageStore::new(PathBuf::from("/docs"), (10, 20));
        assert!(store.worker.is_none());
        store.layout("https://example.com/a.png", 80, 30);
        assert!(store.worker.is_none(), "remote images need no worker");
        store.layout("a.png", 80, 30);
        assert!(store.worker.is_some());
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mdx-images-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_image(path: &Path, w: u32, h: u32, format: image::ImageFormat) {
        let img = image::RgbImage::from_pixel(w, h, image::Rgb([200, 30, 30]));
        img.save_with_format(path, format).unwrap();
    }

    fn wait_for(store: &mut ImageStore) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while store.loading() {
            store.poll();
            assert!(Instant::now() < deadline, "decode did not finish");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn loads_png_and_jpeg_in_the_background() {
        let dir = temp_dir("load");
        write_image(&dir.join("a.png"), 40, 20, image::ImageFormat::Png);
        write_image(&dir.join("b.jpg"), 30, 60, image::ImageFormat::Jpeg);
        let mut store = ImageStore::new(dir.clone(), (10, 20));
        assert_eq!(store.layout("a.png", 80, 30), None, "first call only loads");
        assert_eq!(store.layout("b.jpg", 80, 30), None);
        wait_for(&mut store);
        let a = store.layout("a.png", 80, 30).unwrap();
        let b = store.layout("b.jpg", 80, 30).unwrap();
        assert_eq!((a.cols, a.rows), (4, 1));
        assert_eq!((b.cols, b.rows), (3, 3));
        assert_ne!(a.id, b.id);
        let out = String::from_utf8(store.take_output()).unwrap();
        assert!(out.contains(&format!("a=t,f=100,t=d,i={}", a.id)));
        assert!(out.contains(&format!("a=p,U=1,i={},p=1,c=4,r=1", a.id)));
        assert!(out.contains(&format!("a=p,U=1,i={},p=1,c=3,r=3", b.id)));
        let deleted = String::from_utf8(store.delete_all()).unwrap();
        assert!(deleted.contains(&format!("a=d,d=I,i={}", a.id)));
        assert!(deleted.contains(&format!("a=d,d=I,i={}", b.id)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_unsupported_and_remote_images_fail() {
        let dir = temp_dir("fail");
        std::fs::write(dir.join("notes.png"), "not an image").unwrap();
        let mut store = ImageStore::new(dir.clone(), (10, 20));
        assert_eq!(store.layout("missing.png", 80, 30), None);
        assert_eq!(store.layout("notes.png", 80, 30), None);
        assert_eq!(store.layout("https://example.com/a.png", 80, 30), None);
        wait_for(&mut store);
        assert_eq!(store.layout("missing.png", 80, 30), None);
        assert_eq!(store.layout("notes.png", 80, 30), None);
        assert!(store.take_output().is_empty(), "nothing to send");
        assert!(store.delete_all().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn placement_is_resent_only_when_the_size_changes() {
        let mut store = ImageStore::for_tests(PathBuf::from("/docs"), (10, 20));
        let id = store.insert_ready("a.png", (1000, 400));
        let first = store.layout("a.png", 80, 30).unwrap();
        assert_eq!((first.id, first.cols, first.rows), (id, 80, 16));
        assert!(!store.take_output().is_empty());
        store.layout("a.png", 80, 30).unwrap();
        assert!(
            store.take_output().is_empty(),
            "same size, no new placement"
        );
        let narrow = store.layout("a.png", 40, 30).unwrap();
        assert_eq!((narrow.cols, narrow.rows), (40, 8));
        assert_eq!(store.take_output(), kitty::place(id, 40, 8));
    }

    #[test]
    fn tmux_store_wraps_placements_and_deletes() {
        let mut store = ImageStore::for_tests(PathBuf::from("/docs"), (10, 20)).through_tmux();
        let id = store.insert_ready("a.png", (100, 40));
        store.layout("a.png", 80, 30).unwrap();
        assert_eq!(
            store.take_output(),
            kitty::tmux_passthrough(&kitty::place(id, 10, 2))
        );
        assert_eq!(
            store.delete_all(),
            kitty::tmux_passthrough(&kitty::delete(id))
        );
    }

    #[test]
    fn retain_deletes_unreferenced_images() {
        let mut store = ImageStore::for_tests(PathBuf::from("/docs"), (10, 20));
        let a = store.insert_ready("a.png", (10, 10));
        let b = store.insert_ready("b.png", (10, 10));
        store.take_output();
        store.retain(["a.png"]);
        assert_eq!(store.take_output(), kitty::delete(b));
        assert!(store.layout("a.png", 80, 30).is_some_and(|l| l.id == a));
    }

    #[test]
    fn changed_mtime_reloads_and_retransmits() {
        let dir = temp_dir("mtime");
        let path = dir.join("a.png");
        write_image(&path, 20, 20, image::ImageFormat::Png);
        let mut store = ImageStore::new(dir.clone(), (10, 20));
        store.layout("a.png", 80, 30);
        wait_for(&mut store);
        let id = store.layout("a.png", 80, 30).unwrap().id;
        store.take_output();

        store.refresh();
        assert!(!store.loading(), "unchanged file is not decoded again");

        write_image(&path, 40, 20, image::ImageFormat::Png);
        let later = SystemTime::now() + Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(later)
            .unwrap();
        store.refresh();
        assert!(store.loading());
        // The old image stays until the new one is decoded.
        assert!(store.layout("a.png", 80, 30).is_some());
        wait_for(&mut store);
        let layout = store.layout("a.png", 80, 30).unwrap();
        assert_eq!((layout.id, layout.cols, layout.rows), (id, 4, 1));
        let out = String::from_utf8(store.take_output()).unwrap();
        assert!(out.contains(&format!("a=t,f=100,t=d,i={id}")));
        assert!(out.contains(&format!("a=p,U=1,i={id},p=1,c=4,r=1")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn large_images_are_scaled_before_transmission() {
        let dir = temp_dir("large");
        let path = dir.join("big.png");
        write_image(&path, 3000, 1500, image::ImageFormat::Png);
        let decoded = decode(&path).unwrap();
        assert_eq!(decoded.px, (3000, 1500));
        let sent = image::load_from_memory(&decoded.png).unwrap();
        assert_eq!((sent.width(), sent.height()), (2048, 1024));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn diagram(source: &str) -> crate::mermaid::svg::DiagramImage {
        crate::mermaid::render_diagram(source, crate::theme::Theme::default_theme(), 80)
            .unwrap()
            .image
    }

    #[test]
    fn diagrams_rasterize_in_the_background_and_fill_their_cells() {
        let flowchart = diagram("graph LR\n    A[Start] --> B[End]\n");
        let (cols, rows) = flowchart.cells();
        let mut store = ImageStore::new(PathBuf::from("/docs"), (10, 20));
        assert_eq!(
            store.diagram(&flowchart),
            None,
            "first call only rasterizes"
        );
        assert!(store.loading());
        wait_for(&mut store);
        let layout = store.diagram(&flowchart).unwrap();
        assert_eq!(
            (usize::from(layout.cols), usize::from(layout.rows)),
            (cols, rows)
        );
        let out = String::from_utf8(store.take_output()).unwrap();
        assert!(out.contains(&format!("a=t,f=100,t=d,i={}", layout.id)));
        assert!(out.contains(&format!("a=p,U=1,i={},p=1,c={cols},r={rows}", layout.id)));
        // Cached: asking again neither re-rasterizes nor re-places.
        assert_eq!(store.diagram(&flowchart), Some(layout));
        assert!(!store.loading());
        assert!(store.take_output().is_empty());
        let deleted = String::from_utf8(store.delete_all()).unwrap();
        assert!(deleted.contains(&format!("a=d,d=I,i={}", layout.id)));
    }

    fn math(tex: &str) -> Arc<crate::math::MathImage> {
        Arc::new(crate::math::MathImage::new(
            tex,
            crate::theme::Theme::default_theme(),
        ))
    }

    #[test]
    fn math_typesets_in_the_background_at_its_natural_size() {
        let formula = math(r"\sum_{i=1}^{n} i = \frac{n(n+1)}{2}");
        let mut store = ImageStore::new(PathBuf::from("/docs"), (10, 20));
        assert_eq!(store.math(&formula, 80), None, "first call only typesets");
        assert!(store.loading());
        wait_for(&mut store);
        let layout = store.math(&formula, 80).unwrap();
        assert!(layout.rows >= 2, "{layout:?}");
        let out = String::from_utf8(store.take_output()).unwrap();
        assert!(out.contains(&format!("a=t,f=100,t=d,i={}", layout.id)));
        let (cols, rows) = (layout.cols, layout.rows);
        assert!(out.contains(&format!("a=p,U=1,i={},p=1,c={cols},r={rows}", layout.id)));
        // Cached by source, theme and cell size.
        assert_eq!(store.math(&formula, 80), Some(layout));
        assert!(store.take_output().is_empty());
        // Narrower than the formula: scaled down and placed again.
        let narrow = store.math(&formula, cols - 2).unwrap();
        assert_eq!(narrow.id, layout.id);
        assert!(narrow.cols <= cols - 2 && narrow.rows <= rows);
        assert!(!store.take_output().is_empty());
        // Under half its width: the source text reads better.
        assert_eq!(store.math(&formula, cols / 2 - 1), None);
    }

    #[test]
    fn math_that_fails_to_typeset_keeps_the_source() {
        let formula = math(r"\notarealcommand{x}");
        let mut store = ImageStore::new(PathBuf::from("/docs"), (10, 20));
        assert_eq!(store.math(&formula, 80), None);
        wait_for(&mut store);
        assert_eq!(store.math(&formula, 80), None);
        assert!(!store.loading());
        assert!(store.take_output().is_empty(), "nothing sent");
    }

    #[test]
    fn a_new_cell_size_typesets_math_again() {
        let formula = math(r"\frac{a}{b}");
        let mut store = ImageStore::new(PathBuf::from("/docs"), (10, 20));
        store.math(&formula, 80);
        wait_for(&mut store);
        let small = store.math(&formula, 80).unwrap();
        store.take_output();
        store.set_cell_px((20, 40));
        assert_eq!(
            store.take_output(),
            kitty::delete(small.id),
            "old size deleted"
        );
        // The source shows while the new size typesets.
        assert_eq!(store.math(&formula, 80), None);
        wait_for(&mut store);
        let large = store.math(&formula, 80).unwrap();
        assert_ne!(large.id, small.id);
        // Typeset for the cell size, so it takes the same cells.
        assert_eq!((large.cols, large.rows), (small.cols, small.rows));
    }

    #[test]
    fn math_is_kept_by_retain_diagrams_while_referenced() {
        let formula = math("x");
        let mut store = ImageStore::for_tests(PathBuf::from("/docs"), (10, 20));
        let id = store.insert_ready_math(&formula, (10, 20));
        assert_eq!(store.math(&formula, 80).map(|l| l.id), Some(id));
        store.take_output();
        store.retain_diagrams([formula.key()]);
        assert!(store.take_output().is_empty());
        store.retain_diagrams([]);
        assert_eq!(store.take_output(), kitty::delete(id));
    }

    #[test]
    fn a_new_cell_size_rasterizes_again() {
        let flowchart = diagram("graph LR\n    A --> B\n");
        let mut store = ImageStore::for_tests(PathBuf::from("/docs"), (10, 20));
        let id = store.insert_ready_diagram(&flowchart);
        assert_eq!(store.diagram(&flowchart).map(|l| l.id), Some(id));
        store.take_output();
        store.set_cell_px((20, 40));
        assert_eq!(store.take_output(), kitty::delete(id), "old size deleted");
        store.set_cell_px((20, 40));
        assert!(store.take_output().is_empty(), "same size: nothing to do");
        // No worker in this store, so the new size fails and the pager keeps
        // the ASCII diagram.
        assert_eq!(store.diagram(&flowchart), None);
        assert!(!store.loading());
    }

    #[test]
    fn diagrams_too_large_to_address_stay_ascii() {
        let source = format!(
            "graph LR\n    A[{}] --> B\n",
            "x".repeat(usize::from(kitty::MAX_CELLS))
        );
        let wide = diagram(&source);
        assert!(wide.cells().0 > usize::from(kitty::MAX_CELLS));
        let mut store = ImageStore::new(PathBuf::from("/docs"), (10, 20));
        assert_eq!(store.diagram(&wide), None);
        assert!(!store.loading(), "nothing is rasterized");
        assert!(store.worker.is_none());
    }

    #[test]
    fn diagram_pending_only_while_rasterizing_for_this_cell_size() {
        let a = diagram("graph LR\n    A --> B\n");
        let mut store = ImageStore::for_tests(PathBuf::from("/docs"), (10, 20));
        assert!(!store.diagram_pending(&a));
        let id = store.alloc_id();
        store.diagrams.insert(
            (a.key(), (10, 20)),
            DiagramEntry {
                id,
                status: Status::Pending,
                placed: None,
            },
        );
        assert!(store.diagram_pending(&a));
        store.set_cell_px((8, 16));
        assert!(!store.diagram_pending(&a));
        // Without a worker the request fails at once.
        assert!(store.diagram(&a).is_none());
        assert!(!store.diagram_pending(&a));
    }

    #[test]
    fn retain_diagrams_deletes_unreferenced_ones() {
        let a = diagram("graph LR\n    A --> B\n");
        let b = diagram("graph LR\n    C --> D\n");
        let mut store = ImageStore::for_tests(PathBuf::from("/docs"), (10, 20));
        let a_id = store.insert_ready_diagram(&a);
        let b_id = store.insert_ready_diagram(&b);
        store.take_output();
        store.retain_diagrams([a.key()]);
        assert_eq!(store.take_output(), kitty::delete(b_id));
        assert_eq!(store.diagram(&a).map(|l| l.id), Some(a_id));
    }

    #[test]
    fn ids_stay_within_24_bits_and_skip_zero() {
        let mut store = ImageStore::for_tests(PathBuf::from("/"), (10, 20));
        store.next_id = 0x00ff_ffff;
        assert_eq!(store.alloc_id(), 0x00ff_ffff);
        assert_eq!(store.alloc_id(), 1);
        assert!((1..=0x00ff_ffff).contains(&first_id()));
    }
}
