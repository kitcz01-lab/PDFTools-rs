mod pdf_ops;
mod settings;

use settings::AppSettings;
use slint::{Model, ModelRc, SharedString, VecModel};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

slint::include_modules!();

fn create_file_item(path: &Path) -> FileItem {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("neznámý")
        .to_string();

    let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let size_str = pdf_ops::format_size(size);

    FileItem {
        name: SharedString::from(name),
        size_str: SharedString::from(size_str),
        path: SharedString::from(path.to_string_lossy().to_string()),
    }
}

fn pick_pdf_files(last_dir: &str) -> Option<Vec<PathBuf>> {
    rfd::FileDialog::new()
        .add_filter("PDF dokumenty", &["pdf", "PDF"])
        .set_directory(last_dir)
        .pick_files()
}

fn pick_save_pdf(default_path: &str, last_dir: &str) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new()
        .add_filter("PDF dokumenty", &["pdf", "PDF"])
        .set_directory(last_dir);

    if !default_path.is_empty() {
        if let Some(file_name) = Path::new(default_path).file_name() {
            dialog = dialog.set_file_name(file_name.to_string_lossy());
        }
    }

    dialog.save_file()
}

fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;
    let settings = AppSettings::load();

    // Default output path for merge
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let default_output = PathBuf::from(&home).join("output.pdf");
    main_window.set_merge_output_path(SharedString::from(default_output.to_string_lossy().to_string()));

    main_window.set_merge_open_after(settings.merge_open_after);
    main_window.set_split_open_after(settings.split_open_after);
    main_window.set_compress_open_after(settings.compress_open_after);
    main_window.set_split_at_page(settings.split_at_page);
    main_window.set_compress_quality(settings.compress_quality);

    // In-memory file models
    let merge_model = Rc::new(VecModel::<FileItem>::default());
    main_window.set_merge_files(ModelRc::from(merge_model.clone()));

    let split_model = Rc::new(VecModel::<FileItem>::default());
    main_window.set_split_files(ModelRc::from(split_model.clone()));

    let compress_model = Rc::new(VecModel::<FileItem>::default());
    main_window.set_compress_files(ModelRc::from(compress_model.clone()));

    // ---------------- MERGE CALLBACKS ----------------
    {
        let main_weak = main_window.as_weak();
        let model = merge_model.clone();
        main_window.on_add_merge_files(move || {
            let st = AppSettings::load();
            if let Some(files) = pick_pdf_files(&st.last_dir) {
                if let Some(first) = files.first() {
                    if let Some(parent) = first.parent() {
                        let mut new_st = st.clone();
                        new_st.last_dir = parent.to_string_lossy().to_string();
                        new_st.save();
                    }
                }
                for f in files {
                    model.push(create_file_item(&f));
                }
                if let Some(w) = main_weak.upgrade() {
                    w.set_status_message(SharedString::from(format!("Přidáno souborů: {}", model.row_count())));
                }
            }
        });
    }

    {
        let model = merge_model.clone();
        let main_weak = main_window.as_weak();
        main_window.on_remove_merge_file(move |idx| {
            if idx >= 0 && (idx as usize) < model.row_count() {
                model.remove(idx as usize);
                if let Some(w) = main_weak.upgrade() {
                    w.set_merge_selected_idx(-1);
                }
            }
        });
    }

    {
        let model = merge_model.clone();
        let main_weak = main_window.as_weak();
        main_window.on_move_merge_up(move |idx| {
            let i = idx as usize;
            if i > 0 && i < model.row_count() {
                let item = model.row_data(i).unwrap();
                model.remove(i);
                model.insert(i - 1, item);
                if let Some(w) = main_weak.upgrade() {
                    w.set_merge_selected_idx((i - 1) as i32);
                }
            }
        });
    }

    {
        let model = merge_model.clone();
        let main_weak = main_window.as_weak();
        main_window.on_move_merge_down(move |idx| {
            let i = idx as usize;
            if i < model.row_count() - 1 {
                let item = model.row_data(i).unwrap();
                model.remove(i);
                model.insert(i + 1, item);
                if let Some(w) = main_weak.upgrade() {
                    w.set_merge_selected_idx((i + 1) as i32);
                }
            }
        });
    }

    {
        let model = merge_model.clone();
        let main_weak = main_window.as_weak();
        main_window.on_clear_merge(move || {
            while model.row_count() > 0 {
                model.remove(0);
            }
            if let Some(w) = main_weak.upgrade() {
                w.set_merge_selected_idx(-1);
                w.set_status_message(SharedString::from("Seznam vyprázdněn."));
            }
        });
    }

    {
        let main_weak = main_window.as_weak();
        main_window.on_browse_merge_output(move || {
            if let Some(w) = main_weak.upgrade() {
                let cur = w.get_merge_output_path().to_string();
                let st = AppSettings::load();
                if let Some(picked) = pick_save_pdf(&cur, &st.last_dir) {
                    let mut final_path = picked;
                    if final_path.extension().map_or(true, |ext| ext != "pdf" && ext != "PDF") {
                        final_path.set_extension("pdf");
                    }
                    w.set_merge_output_path(SharedString::from(final_path.to_string_lossy().to_string()));
                }
            }
        });
    }

    {
        let main_weak = main_window.as_weak();
        let model = merge_model.clone();
        main_window.on_run_merge(move || {
            let w = match main_weak.upgrade() {
                Some(w) => w,
                None => return,
            };

            let out_str = w.get_merge_output_path().to_string();
            let open_after = w.get_merge_open_after();
            let mut paths = Vec::new();
            for i in 0..model.row_count() {
                if let Some(item) = model.row_data(i) {
                    paths.push(PathBuf::from(item.path.as_str()));
                }
            }

            if paths.len() < 2 {
                w.set_status_message(SharedString::from("Vyberte alespoň 2 soubory ke sloučení."));
                return;
            }

            w.set_is_busy(true);
            w.set_status_message(SharedString::from("Probíhá slučování PDF dokumentů..."));
            w.set_progress_value(0.5);

            let out_path = PathBuf::from(out_str);
            let weak_clone = main_weak.clone();

            std::thread::spawn(move || {
                let res = pdf_ops::merge_pdfs(&paths, &out_path);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = weak_clone.upgrade() {
                        win.set_is_busy(false);
                        win.set_progress_value(1.0);
                        match res {
                            Ok(()) => {
                                win.set_status_message(SharedString::from("Sloučení úspěšně dokončeno!"));
                                if open_after {
                                    let _ = open::that(&out_path);
                                }
                            }
                            Err(e) => {
                                win.set_status_message(SharedString::from(format!("Chyba: {}", e)));
                            }
                        }
                    }
                });
            });
        });
    }

    // ---------------- SPLIT CALLBACKS ----------------
    {
        let main_weak = main_window.as_weak();
        let model = split_model.clone();
        main_window.on_add_split_files(move || {
            let st = AppSettings::load();
            if let Some(files) = pick_pdf_files(&st.last_dir) {
                for f in files {
                    model.push(create_file_item(&f));
                }
                if let Some(w) = main_weak.upgrade() {
                    w.set_status_message(SharedString::from(format!("Vybráno k rozdělení: {}", model.row_count())));
                }
            }
        });
    }

    {
        let model = split_model.clone();
        let main_weak = main_window.as_weak();
        main_window.on_remove_split_file(move |idx| {
            if idx >= 0 && (idx as usize) < model.row_count() {
                model.remove(idx as usize);
                if let Some(w) = main_weak.upgrade() {
                    w.set_split_selected_idx(-1);
                }
            }
        });
    }

    {
        let model = split_model.clone();
        let main_weak = main_window.as_weak();
        main_window.on_clear_split(move || {
            while model.row_count() > 0 {
                model.remove(0);
            }
            if let Some(w) = main_weak.upgrade() {
                w.set_split_selected_idx(-1);
            }
        });
    }

    {
        let main_weak = main_window.as_weak();
        main_window.on_change_split_page(move |delta| {
            if let Some(w) = main_weak.upgrade() {
                let cur = w.get_split_at_page();
                let next = (cur + delta).max(1);
                w.set_split_at_page(next);
                let mut st = AppSettings::load();
                st.split_at_page = next;
                st.save();
            }
        });
    }

    {
        let main_weak = main_window.as_weak();
        let model = split_model.clone();
        main_window.on_run_split(move || {
            let w = match main_weak.upgrade() {
                Some(w) => w,
                None => return,
            };

            let split_at = w.get_split_at_page() as usize;
            let open_after = w.get_split_open_after();
            let mut paths = Vec::new();
            for i in 0..model.row_count() {
                if let Some(item) = model.row_data(i) {
                    paths.push(PathBuf::from(item.path.as_str()));
                }
            }

            if paths.is_empty() {
                w.set_status_message(SharedString::from("Vyberte alespoň 1 soubor k rozdělení."));
                return;
            }

            w.set_is_busy(true);
            w.set_status_message(SharedString::from("Probíhá rozdělování dokumentů..."));

            let weak_clone = main_weak.clone();

            std::thread::spawn(move || {
                let mut all_generated = Vec::new();
                let mut errors = Vec::new();

                for p in &paths {
                    match pdf_ops::split_pdf(p, split_at) {
                        Ok(files) => all_generated.extend(files),
                        Err(e) => errors.push(e),
                    }
                }

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = weak_clone.upgrade() {
                        win.set_is_busy(false);
                        if errors.is_empty() {
                            win.set_status_message(SharedString::from(format!(
                                "Rozdělení dokončeno! Vytvořeno {} souborů.",
                                all_generated.len()
                            )));
                            if open_after {
                                if let Some(first_file) = all_generated.first() {
                                    if let Some(parent) = first_file.parent() {
                                        let _ = open::that(parent);
                                    }
                                }
                            }
                        } else {
                            win.set_status_message(SharedString::from(format!("Chyby: {}", errors.join("; "))));
                        }
                    }
                });
            });
        });
    }

    // ---------------- COMPRESS CALLBACKS ----------------
    {
        let main_weak = main_window.as_weak();
        let model = compress_model.clone();
        main_window.on_add_compress_files(move || {
            let st = AppSettings::load();
            if let Some(files) = pick_pdf_files(&st.last_dir) {
                for f in files {
                    model.push(create_file_item(&f));
                }
                if let Some(w) = main_weak.upgrade() {
                    w.set_status_message(SharedString::from(format!("Vybráno ke kompresi: {}", model.row_count())));
                }
            }
        });
    }

    {
        let model = compress_model.clone();
        let main_weak = main_window.as_weak();
        main_window.on_remove_compress_file(move |idx| {
            if idx >= 0 && (idx as usize) < model.row_count() {
                model.remove(idx as usize);
                if let Some(w) = main_weak.upgrade() {
                    w.set_compress_selected_idx(-1);
                }
            }
        });
    }

    {
        let model = compress_model.clone();
        let main_weak = main_window.as_weak();
        main_window.on_clear_compress(move || {
            while model.row_count() > 0 {
                model.remove(0);
            }
            if let Some(w) = main_weak.upgrade() {
                w.set_compress_selected_idx(-1);
            }
        });
    }

    {
        let main_weak = main_window.as_weak();
        let model = compress_model.clone();
        main_window.on_run_compress(move || {
            let w = match main_weak.upgrade() {
                Some(w) => w,
                None => return,
            };

            let quality = w.get_compress_quality() as u8;
            let open_after = w.get_compress_open_after();
            let mut paths = Vec::new();
            for i in 0..model.row_count() {
                if let Some(item) = model.row_data(i) {
                    paths.push(PathBuf::from(item.path.as_str()));
                }
            }

            if paths.is_empty() {
                w.set_status_message(SharedString::from("Vyberte alespoň 1 soubor ke kompresi."));
                return;
            }

            w.set_is_busy(true);
            w.set_status_message(SharedString::from("Probíhá komprese dokumentů..."));

            let weak_clone = main_weak.clone();

            std::thread::spawn(move || {
                let mut total_orig = 0u64;
                let mut total_comp = 0u64;
                let mut errors = Vec::new();
                let mut output_dirs = Vec::new();

                for p in &paths {
                    let parent = p.parent().unwrap_or_else(|| Path::new("."));
                    let out_dir = parent.join("output");
                    let out_file = out_dir.join(p.file_name().unwrap_or_default());
                    if !output_dirs.contains(&out_dir) {
                        output_dirs.push(out_dir);
                    }

                    match pdf_ops::compress_pdf(p, quality, &out_file) {
                        Ok((orig, comp)) => {
                            total_orig += orig;
                            total_comp += comp;
                        }
                        Err(e) => errors.push(e),
                    }
                }

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = weak_clone.upgrade() {
                        win.set_is_busy(false);
                        if errors.is_empty() {
                            let ratio = if total_orig > 0 {
                                (100.0 * total_comp as f64) / (total_orig as f64)
                            } else {
                                100.0
                            };
                            let msg = format!(
                                "Komprese dokončena! Původní: {}, Nová: {} ({:.1} %)",
                                pdf_ops::format_size(total_orig),
                                pdf_ops::format_size(total_comp),
                                ratio
                            );
                            win.set_status_message(SharedString::from(msg));

                            if open_after {
                                if let Some(dir) = output_dirs.first() {
                                    let _ = open::that(dir);
                                }
                            }
                        } else {
                            win.set_status_message(SharedString::from(format!("Chyby: {}", errors.join("; "))));
                        }
                    }
                });
            });
        });
    }

    main_window.run()
}
