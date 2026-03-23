use crate::edit_state::EditState;
use crate::rpyc_compiler;
use rpa_lib::{RpaArchive, RpaPacker};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum PackingProgress {
    Started { total: usize },
    FileDone { index: usize, filename: String },
    RpycCompiling { filename: String },
    RpycCompiled { filename: String },
    RpycFailed { filename: String, reason: String },
    Error(String),
    Finished,
}

/// Compile any .rpyc files that need recompilation due to .rpy edits.
/// Returns a map of rpyc_filename -> compiled bytes.
/// Files that fail to compile are added to the `remove` set instead.
fn compile_rpyc_files(
    edit_state: &EditState,
    sender: &async_channel::Sender<PackingProgress>,
) -> (HashMap<String, Vec<u8>>, Vec<String>) {
    let mut compiled = HashMap::new();
    let mut failed = Vec::new();

    for rpyc_name in &edit_state.rpyc_recompile {
        // Find the corresponding .rpy source from text_edits
        let rpy_name = match rpyc_compiler::rpy_counterpart(rpyc_name) {
            Some(name) => name,
            None => continue,
        };

        let rpy_source = match edit_state.text_edits.get(&rpy_name) {
            Some(source) => source,
            None => continue, // .rpy wasn't actually edited, skip
        };

        let _ = sender.send_blocking(PackingProgress::RpycCompiling {
            filename: rpyc_name.clone(),
        });

        match rpyc_compiler::compile_rpy_to_rpyc(rpy_source, &rpy_name) {
            Ok(rpyc_bytes) => {
                let _ = sender.send_blocking(PackingProgress::RpycCompiled {
                    filename: rpyc_name.clone(),
                });
                compiled.insert(rpyc_name.clone(), rpyc_bytes);
            }
            Err(reason) => {
                let _ = sender.send_blocking(PackingProgress::RpycFailed {
                    filename: rpyc_name.clone(),
                    reason: reason.clone(),
                });
                failed.push(rpyc_name.clone());
            }
        }
    }

    (compiled, failed)
}

pub fn pack_archive_background(
    archive: Option<Arc<RpaArchive>>,
    edit_state: EditState,
    output_path: PathBuf,
    sender: async_channel::Sender<PackingProgress>,
) {
    // Compile .rpyc files from edited .rpy sources
    let (compiled_rpyc, failed_rpyc) = compile_rpyc_files(&edit_state, &sender);

    // Merge compiled rpyc into text_edits as raw bytes, and remove failed ones
    let mut removed = edit_state.removed.clone();
    let mut extra_memory: HashMap<String, Vec<u8>> = HashMap::new();

    for (rpyc_name, rpyc_bytes) in compiled_rpyc {
        extra_memory.insert(rpyc_name, rpyc_bytes);
    }

    // Remove .rpyc files that failed to compile (Ren'Py will regenerate them)
    for rpyc_name in failed_rpyc {
        removed.insert(rpyc_name);
    }

    let packer = if let Some(archive) = archive.as_ref() {
        let mut packer = rpa_lib::packer::packer_from_archive_with_edits(
            archive,
            &removed,
            &edit_state.replaced,
            &edit_state.added,
            &edit_state.text_edits,
        );

        // Add compiled .rpyc files as memory entries
        for (rpyc_name, rpyc_bytes) in &extra_memory {
            packer.add_entry(
                rpyc_name.clone(),
                rpa_lib::DataSource::FromMemory(rpyc_bytes.clone()),
            );
        }

        packer
    } else {
        // New archive from added files only
        let mut packer = RpaPacker::new(rpa_lib::RpaVersion::V3_2);
        for (filename, path) in &edit_state.added {
            packer.add_entry(filename.clone(), rpa_lib::DataSource::FromFile(path.clone()));
        }
        packer
    };

    match packer.pack(&output_path, |current, total, filename| {
        if current < total {
            let _ = sender.send_blocking(PackingProgress::FileDone {
                index: current,
                filename: filename.to_string(),
            });
        } else {
            let _ = sender.send_blocking(PackingProgress::Finished);
        }
    }) {
        Ok(()) => {
            let _ = sender.send_blocking(PackingProgress::Finished);
        }
        Err(e) => {
            let _ = sender.send_blocking(PackingProgress::Error(e.to_string()));
        }
    }
}
