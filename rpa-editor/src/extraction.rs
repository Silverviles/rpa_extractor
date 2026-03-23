use rpa_lib::RpaArchive;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum ExtractionProgress {
    Started { total: usize },
    FileDone { index: usize, filename: String },
    Error { filename: String, error: String },
    Finished,
}

pub fn extract_files_background(
    archive: Arc<RpaArchive>,
    filenames: Vec<String>,
    dest_dir: PathBuf,
    sender: async_channel::Sender<ExtractionProgress>,
) {
    let total = filenames.len();
    let _ = sender.send_blocking(ExtractionProgress::Started { total });

    for (i, filename) in filenames.iter().enumerate() {
        let dest = dest_dir.join(filename);
        match archive.extract_to(filename, &dest) {
            Ok(()) => {
                let _ = sender.send_blocking(ExtractionProgress::FileDone {
                    index: i,
                    filename: filename.clone(),
                });
            }
            Err(e) => {
                let _ = sender.send_blocking(ExtractionProgress::Error {
                    filename: filename.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    let _ = sender.send_blocking(ExtractionProgress::Finished);
}
