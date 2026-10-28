use lopdf::{dictionary, Dictionary, Document, Error, Object, ObjectId};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} kB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn inherited_page_attribute(doc: &Document, page_id: ObjectId, key: &[u8]) -> lopdf::Result<Option<Object>> {
    let mut current = page_id;
    let mut visited = HashSet::new();
    loop {
        if !visited.insert(current) {
            return Err(Error::ReferenceCycle(current));
        }
        let node = doc.get_dictionary(current)?;
        if let Ok(value) = node.get(key) {
            match doc.dereference(value) {
                Ok((_, Object::Null)) | Err(Error::ObjectNotFound(_)) => {}
                Ok(_) => return Ok(Some(value.clone())),
                Err(error) => return Err(error),
            }
        }
        match node.get(b"Parent") {
            Err(_) | Ok(Object::Null) => return Ok(None),
            Ok(parent) => current = parent.as_reference()?,
        }
    }
}

fn materialize_page_attributes(doc: &Document, page_id: ObjectId) -> lopdf::Result<Dictionary> {
    let mut page = doc.get_dictionary(page_id)?.clone();
    let media_box = inherited_page_attribute(doc, page_id, b"MediaBox")?.unwrap_or_else(|| {
        Object::Array(vec![0.into(), 0.into(), 595.into(), 842.into()])
    });
    let crop_box = inherited_page_attribute(doc, page_id, b"CropBox")?.unwrap_or_else(|| media_box.clone());
    let resources = inherited_page_attribute(doc, page_id, b"Resources")?.unwrap_or_else(|| Object::Dictionary(Dictionary::new()));
    let rotation = inherited_page_attribute(doc, page_id, b"Rotate")?.unwrap_or(Object::Integer(0));
    page.set("MediaBox", media_box);
    page.set("CropBox", crop_box);
    page.set("Resources", resources);
    page.set("Rotate", rotation);
    Ok(page)
}

pub fn merge_pdfs(files: &[PathBuf], output: &Path) -> Result<(), String> {
    if files.is_empty() {
        return Err("Seznam souborů ke sloučení je prázdný.".to_string());
    }

    let mut max_id = 1;
    let mut documents_pages = Vec::new();
    let mut documents_objects = BTreeMap::new();
    let mut document = Document::with_version("1.5");

    for file in files {
        let mut doc = Document::load(file)
            .map_err(|e| format!("Chyba při načítání souboru {}: {}", file.display(), e))?;

        let pages = doc.get_pages();
        for (&_page_num, &page_id) in pages.iter() {
            if let Ok(mat_dict) = materialize_page_attributes(&doc, page_id) {
                if let Ok(dict_mut) = doc.get_dictionary_mut(page_id) {
                    *dict_mut = mat_dict;
                }
            }
        }

        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;

        let renumbered_pages = doc.get_pages();
        for (&_page_num, &page_id) in renumbered_pages.iter() {
            documents_pages.push(page_id);
        }
        documents_objects.extend(doc.objects);
    }

    let pages_id = (max_id, 0);
    max_id += 1;

    let mut kids = Vec::new();
    for page_id in documents_pages {
        if let Some(Object::Dictionary(page_dict)) = documents_objects.get_mut(&page_id) {
            page_dict.set("Parent", pages_id);
        }
        kids.push(Object::Reference(page_id));
    }

    let pages_object = dictionary! {
        "Type" => "Pages",
        "Kids" => kids.clone(),
        "Count" => kids.len() as i64,
    };
    documents_objects.insert(pages_id, Object::Dictionary(pages_object));

    let catalog_id = (max_id, 0);
    let catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    };
    documents_objects.insert(catalog_id, Object::Dictionary(catalog));

    document.objects = documents_objects;
    document.trailer.set("Root", catalog_id);
    document.max_id = max_id;

    if let Some(parent_dir) = output.parent() {
        if !parent_dir.exists() {
            let _ = fs::create_dir_all(parent_dir);
        }
    }

    document.save(output)
        .map_err(|e| format!("Chyba při ukládání sloučeného PDF: {}", e))?;

    Ok(())
}

pub fn split_pdf(input: &Path, split_at: usize) -> Result<Vec<PathBuf>, String> {
    if split_at == 0 {
        return Err("Počet stránek musí být alespoň 1.".to_string());
    }

    let orig_doc = Document::load(input)
        .map_err(|e| format!("Chyba při otevírání {}: {}", input.display(), e))?;

    let total_pages = orig_doc.get_pages().len();
    if total_pages == 0 {
        return Err(format!("Dokument {} neobsahuje žádné stránky.", input.display()));
    }

    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("document");
    let parent_dir = input.parent().unwrap_or_else(|| Path::new("."));
    let out_dir = parent_dir.join(stem.to_uppercase());
    if !out_dir.exists() {
        fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    }

    let mut generated_files = Vec::new();
    let num_parts = (total_pages + split_at - 1) / split_at;

    for part_idx in 0..num_parts {
        let p_start = (part_idx * split_at + 1) as u32;
        let p_end = std::cmp::min((part_idx + 1) * split_at, total_pages) as u32;

        let pages_to_delete: Vec<u32> = (1..=total_pages as u32)
            .filter(|&p| p < p_start || p > p_end)
            .collect();

        let mut part_doc = orig_doc.clone();
        part_doc.delete_pages(&pages_to_delete);
        part_doc.prune_objects();

        let part_filename = format!("{}_{:02}.pdf", stem, part_idx + 1);
        let out_file = out_dir.join(part_filename);

        part_doc.save(&out_file)
            .map_err(|e| format!("Chyba při ukládání části {}: {}", part_idx + 1, e))?;

        generated_files.push(out_file);
    }

    Ok(generated_files)
}

pub fn compress_pdf(input: &Path, quality: u8, output: &Path) -> Result<(u64, u64), String> {
    let orig_size = fs::metadata(input).map(|m| m.len()).unwrap_or(0);

    let mut doc = Document::load(input)
        .map_err(|e| format!("Chyba při otevírání {}: {}", input.display(), e))?;

    // Iterujeme přes všechny objekty a hledáme bitmapové obrázky
    for (_obj_id, obj) in doc.objects.iter_mut() {
        if let Object::Stream(ref mut stream) = obj {
            let is_image = stream.dict.get(b"Subtype")
                .ok()
                .and_then(|v| v.as_name().ok())
                .map(|name| name == b"Image")
                .unwrap_or(false);

            if is_image {
                if let Ok(decompressed) = stream.decompressed_content() {
                    if let Ok(dyn_img) = image::load_from_memory(&decompressed) {
                        let mut jpeg_bytes = Vec::new();
                        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_bytes, quality);
                        if encoder.encode_image(&dyn_img).is_ok() {
                            if !jpeg_bytes.is_empty() && (jpeg_bytes.len() < stream.content.len() || stream.content.is_empty()) {
                                stream.set_content(jpeg_bytes);
                                stream.dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
                                stream.dict.remove(b"DecodeParms");
                            }
                        }
                    }
                }
            }
        }
    }

    // Dodatečně zkomprimujeme všechny streamy
    doc.compress();

    if let Some(parent) = output.parent() {
        if !parent.exists() {
            let _ = fs::create_dir_all(parent);
        }
    }

    doc.save(output)
        .map_err(|e| format!("Chyba při ukládání zkomprimovaného PDF: {}", e))?;

    let new_size = fs::metadata(output).map(|m| m.len()).unwrap_or(0);
    Ok((orig_size, new_size))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::Stream;

    fn make_test_pdf(text: &str) -> Document {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 24.into()]),
                Operation::new("Td", vec![100.into(), 700.into()]),
                Operation::new("Tj", vec![Object::string_literal(text)]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        doc
    }

    #[test]
    fn test_merge_and_split() {
        let mut d1 = make_test_pdf("Dokument 1");
        let mut d2 = make_test_pdf("Dokument 2");
        let p1 = PathBuf::from("/tmp/test_d1.pdf");
        let p2 = PathBuf::from("/tmp/test_d2.pdf");
        let out_m = PathBuf::from("/tmp/test_merged.pdf");

        d1.save(&p1).unwrap();
        d2.save(&p2).unwrap();

        merge_pdfs(&[p1.clone(), p2.clone()], &out_m).unwrap();

        let merged = Document::load(&out_m).unwrap();
        assert_eq!(merged.get_pages().len(), 2);

        let parts = split_pdf(&out_m, 1).unwrap();
        assert_eq!(parts.len(), 2);

        let _ = fs::remove_file(p1);
        let _ = fs::remove_file(p2);
        let _ = fs::remove_file(out_m);
        for p in parts {
            let _ = fs::remove_file(p);
        }
    }
}
