//! Model-specific dashboard artwork with bounded, lazy native image residency.
//! A photograph or catalogue entry never enables device controls.
use super::*;
use secbird_core::device_catalog::{Catalog, Resolution};
use serde::Deserialize;
use std::{collections::{BTreeMap, VecDeque}, io::Read};
use windows_sys::Win32::Security::Cryptography::{BCryptHash, BCRYPT_SHA256_ALG_HANDLE};

const MAX_RESIDENT: usize = 8 * 1024 * 1024;
const MAX_CACHE_ENTRIES: usize = 12;
const MAX_INDEX: usize = 1024 * 1024;

#[derive(Clone, Deserialize)]
struct Image {
    product_id: u16,
    edition_id: u16,
    layout_id: u16,
    file: String,
    width: u32,
    height: u32,
    bytes: usize,
    sha256: String,
}

#[derive(Deserialize)]
struct Index { format: String, catalog_sha256: String, images: Vec<Image> }

struct Cached { key: (u16, u16), bitmap: Option<Bitmap>, bytes: usize }

pub(super) struct View {
    root: PathBuf,
    catalog: Option<Catalog>,
    images: BTreeMap<(u16, u16), Image>,
    cache: VecDeque<Cached>,
    resident: usize,
    error: Option<String>,
}

fn read_bounded(path: &Path, max: usize) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path).ok()?;
    if !file.metadata().ok()?.is_file() { return None; }
    let mut data = Vec::new();
    file.take(max as u64 + 1).read_to_end(&mut data).ok()?;
    (data.len() <= max).then_some(data)
}

fn digest(data: &[u8]) -> Option<String> {
    let mut result = [0u8; 32];
    let status = unsafe { BCryptHash(BCRYPT_SHA256_ALG_HANDLE, null(), 0,
        data.as_ptr(), u32::try_from(data.len()).ok()?, result.as_mut_ptr(), 32) };
    (status >= 0).then(|| result.iter().map(|byte| format!("{byte:02x}")).collect())
}

unsafe fn load_png(path: &Path, width: u32, height: u32) -> Option<Bitmap> {
    use windows_sys::Win32::Graphics::GdiPlus as gp;
    let mut image = null_mut();
    if gp::GdipCreateBitmapFromFile(wide(&path.to_string_lossy()).as_ptr(), &mut image) != gp::Ok {
        image_render::dispose(image);
        return None;
    }
    let bitmap = Bitmap { handle: null_mut(), width: width as i32,
        height: height as i32, alpha: true, smooth: image };
    let mut actual_width = 0; let mut actual_height = 0;
    if gp::GdipGetImageWidth(image.cast(), &mut actual_width) != gp::Ok
        || gp::GdipGetImageHeight(image.cast(), &mut actual_height) != gp::Ok
        || (actual_width, actual_height) != (width, height) { return None; }
    Some(bitmap)
}

impl View {
    pub(super) fn load(root: PathBuf) -> Self {
        let mut view = Self { root, catalog: None, images: BTreeMap::new(),
            cache: VecDeque::new(), resident: 0, error: None };
        if let Err(error) = view.read_index() {
            view.catalog = None;
            view.images.clear();
            view.error = Some(error);
        }
        view
    }

    fn read_index(&mut self) -> Result<(), String> {
        let data = read_bounded(&self.root.join("index.json"), MAX_INDEX)
            .ok_or("Dashboard pack index unavailable or exceeds budget")?;
        let index: Index = serde_json::from_slice(&data).map_err(|error| error.to_string())?;
        if index.format != "synapse-lite-dashboard-pack-v2" || index.images.len() > 4096 {
            return Err("Unsupported dashboard pack format or record count".into());
        }
        let catalog = read_bounded(&self.root.join("catalog.json"), 2 * 1024 * 1024)
            .ok_or("Dashboard catalogue unavailable or exceeds budget")?;
        if digest(&catalog).as_deref() != Some(index.catalog_sha256.as_str()) {
            return Err("Dashboard catalogue digest differs from index".into());
        }
        self.catalog = Some(Catalog::parse(std::str::from_utf8(&catalog)
            .map_err(|error| error.to_string())?).map_err(|error| error.to_string())?);
        for image in index.images {
            let expected = format!("dashboard-{}.png", image.sha256);
            if image.file != expected || image.layout_id > 1 || image.width == 0 || image.height == 0
                || image.width > 768 || image.height > 512
                || image.bytes < 33 || image.bytes > 2 * 1024 * 1024
                || image.sha256.len() != 64 || !image.sha256.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err("Invalid dashboard image declaration".into());
            }
            if self.images.insert((image.product_id, image.edition_id), image).is_some() {
                return Err("Conflicting dashboard image identities".into());
            }
        }
        Ok(())
    }

    unsafe fn image(&mut self, connection: u16, edition: u16) -> Option<&Bitmap> {
        let primary = match self.catalog.as_ref()?.resolve(connection) {
            Resolution::Unique(primary) => primary,
            Resolution::Unknown | Resolution::Ambiguous(_) => return None,
        };
        let key = (primary, edition);
        if let Some(position) = self.cache.iter().position(|entry| entry.key == key) {
            let entry = self.cache.remove(position)?;
            self.cache.push_back(entry);
        } else {
            let record = self.images.get(&key)?.clone();
            let needed = record.width as usize * record.height as usize * 4;
            while self.cache.len() >= MAX_CACHE_ENTRIES || self.resident + needed > MAX_RESIDENT {
                let oldest = self.cache.pop_front()?;
                self.resident -= oldest.bytes;
            }
            // Free cold images before admitting the next decoded image. No bulk
            // image preload, network request or device call occurs during paint.
            let bitmap = read_bounded(&self.root.join(&record.file), record.bytes)
                .filter(|data| data.len() == record.bytes)
                .filter(|data| digest(data).is_some_and(|hash| hash.eq_ignore_ascii_case(&record.sha256)))
                .and_then(|data| {
                    let valid_header = data[..8] == *b"\x89PNG\r\n\x1a\n"
                        && data[12..16] == *b"IHDR"
                        && u32::from_be_bytes(data[16..20].try_into().ok()?) == record.width
                        && u32::from_be_bytes(data[20..24].try_into().ok()?) == record.height
                        && data[24] == 8 && data[25] == 6;
                    drop(data);
                    if !valid_header { return None; }
                    load_png(&self.root.join(&record.file), record.width, record.height)
                });
            let bytes = if bitmap.is_some() { needed } else { 0 };
            self.resident += bytes;
            self.cache.push_back(Cached { key, bitmap, bytes });
        }
        self.cache.back()?.bitmap.as_ref()
    }

    pub(super) unsafe fn draw(&mut self, painter: &Painter, device: &Value,
        x: i32, y: i32, width: i32, height: i32) -> bool {
        let Some(connection) = device["pid"].as_str().and_then(|pid| u16::from_str_radix(pid, 16).ok()) else { return false; };
        // The current engine does not report editions. Stock's edition-zero
        // dashboard fallback is artwork only, never a physical identity claim.
        let edition = match device.get("edition_id") {
            None | Some(Value::Null) => 0,
            Some(value) => match value.as_u64().and_then(|id| u16::try_from(id).ok()) {
                Some(id) => id, None => return false,
            },
        };
        let Some(image) = self.image(connection, edition) else { return false; };
        painter.image(Some(image), x, y, width, height);
        true
    }

    pub(super) fn snapshot(&self) -> Value {
        json!({"indexed_images":self.images.len(), "cached_entries":self.cache.len(),
            "resident_pixel_bytes":self.resident, "pixel_budget_bytes":MAX_RESIDENT,
            "error":self.error, "cache":self.cache.iter().map(|entry|json!({
                "product_id":entry.key.0,"edition_id":entry.key.1,
                "loaded":entry.bitmap.is_some(),"pixel_bytes":entry.bytes})).collect::<Vec<_>>()})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder() -> PathBuf {
        let root = std::env::temp_dir().join(format!("dashboard-art-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn fixture(root: &Path, count: u16, width: u32, height: u32) {
        let catalog = serde_json::to_vec(&(1..=count).map(|id|json!({"productId":id,
            "dongleId":if id <= 2 {900} else {id + 1000},"bleId":id+2000})).collect::<Vec<_>>()).unwrap();
        std::fs::write(root.join("catalog.json"), &catalog).unwrap();
        let mut images = Vec::new();
        for id in 1..=count {
            let data: &[u8] = match (width, height) {
                (2, 1) => include_bytes!("../../tests/fixtures/dashboard-tiny.png"),
                (768, 512) => include_bytes!("../../tests/fixtures/dashboard-large.png"),
                _ => panic!("unknown synthetic image dimensions"),
            };
            let file = format!("dashboard-{}.png", digest(data).unwrap());
            std::fs::write(root.join(&file), data).unwrap();
            images.push(json!({"product_id":id,"edition_id":0,"layout_id":0,"file":file,
                "width":width,"height":height,"bytes":data.len(),"sha256":digest(&data).unwrap()}));
        }
        std::fs::write(root.join("index.json"),serde_json::to_vec(&json!({
            "format":"synapse-lite-dashboard-pack-v2","catalog_sha256":digest(&catalog).unwrap(),"images":images})).unwrap()).unwrap();
    }

    #[test]
    fn exact_product_or_unique_alias_selects_art_without_edition_substitution() { unsafe {
        let _runtime = image_render::Runtime::start().unwrap();
        let root = folder(); fixture(&root, 2, 2, 1);
        let mut view = View::load(root);
        assert!(view.error.is_none()); assert_eq!(view.resident, 0);
        assert!(view.image(2001, 0).is_some());
        let bitmap = view.image(2001, 0).unwrap();
        let mut color = 0;
        assert_eq!(windows_sys::Win32::Graphics::GdiPlus::GdipBitmapGetPixel(bitmap.smooth, 0, 0, &mut color), 0);
        assert_eq!(color >> 24, 128, "PNG alpha must survive native admission");
        for (shift, expected) in [(16, 100u32), (8, 150), (0, 200)] {
            assert!(((color >> shift) & 255).abs_diff(expected) <= 1, "No matte or double premultiplication");
        }
        assert_eq!(view.cache.back().unwrap().key, (1, 0));
        assert!(view.image(900, 0).is_none(), "shared receiver must not select the first photo");
        assert!(view.image(1, 128).is_none(), "unknown edition must not borrow another edition");
        assert!(view.image(999, 0).is_none());
        assert!(view.image(2, 0).is_some());
        assert_eq!(view.resident, 16);
    }}

    #[test]
    fn lazy_cache_evicts_before_admitting_more_than_the_pixel_budget() { unsafe {
        let _runtime = image_render::Runtime::start().unwrap();
        let root = folder(); fixture(&root, 8, 768, 512);
        let mut view = View::load(root);
        assert_eq!(view.images.len(), 8); assert_eq!(view.resident, 0);
        for _ in 0..2 { for id in 1..=8 {
            assert!(view.image(id, 0).is_some());
            assert!(view.resident <= MAX_RESIDENT);
            assert!(view.cache.len() <= MAX_CACHE_ENTRIES);
            assert_eq!(view.resident, view.cache.iter().map(|entry|entry.bytes).sum::<usize>());
        }}
        assert!(!view.cache.iter().any(|entry|entry.key == (1, 0)));
        assert!(view.cache.len() < view.images.len());
    }}

    #[test]
    fn corruption_and_invalid_pack_paths_cannot_load_pixels() { unsafe {
        let _runtime = image_render::Runtime::start().unwrap();
        let root = folder(); fixture(&root, 1, 2, 1);
        let index: Value = serde_json::from_slice(&std::fs::read(root.join("index.json")).unwrap()).unwrap();
        let image = root.join(index["images"][0]["file"].as_str().unwrap());
        let mut bytes = std::fs::read(&image).unwrap(); bytes[16] ^= 1;
        std::fs::write(&image, bytes).unwrap();
        let mut view = View::load(root.clone());
        assert!(view.image(1, 0).is_none()); assert_eq!(view.resident, 0);
        assert_eq!(view.cache.len(), 1, "failed image admission is cached too");
        let path = root.join("index.json");
        let mut index: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        index["images"][0]["file"] = json!("../elsewhere.png");
        std::fs::write(&path, serde_json::to_vec(&index).unwrap()).unwrap();
        let view = View::load(root);
        assert!(view.error.is_some()); assert!(view.images.is_empty());
    }}

    #[test]
    #[ignore = "requires explicitly provisioned private device-pack; decodes all captured native images"]
    fn provisioned_pack_loads_every_declared_image_with_bounded_residency() { unsafe {
        let _runtime = image_render::Runtime::start().unwrap();
        let mut view = View::load(assets_dir().join("device-pack"));
        assert!(view.error.is_none(), "{:?}", view.error);
        let keys: Vec<_> = view.images.keys().copied().collect();
        assert!(!keys.is_empty()); assert_eq!(view.resident, 0);
        for (pid, edition) in &keys {
            let image = view.image(*pid, *edition).unwrap_or_else(||panic!("{pid}/{edition}"));
            let mut pixel = 0;
            assert_eq!(windows_sys::Win32::Graphics::GdiPlus::GdipBitmapGetPixel(
                image.smooth, image.width - 1, image.height - 1, &mut pixel), 0);
            assert!(view.resident <= MAX_RESIDENT);
        }
        eprintln!("Validated {} native dashboard variants; final pixel residency {} bytes", keys.len(), view.resident);
    }}
}
