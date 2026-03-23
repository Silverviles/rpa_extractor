use crate::error::RpaError;
use crate::header::{self, RpaHeader, RpaVersion};
use crate::index::{self, IndexEntry};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub struct RpaArchive {
    path: PathBuf,
    header: RpaHeader,
    entries: HashMap<String, IndexEntry>,
}

impl RpaArchive {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RpaError> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path)?;
        let rpa_header = header::parse_header(&mut file)?;

        // Re-open to parse index (parse_header consumed part of the BufReader)
        let mut file = File::open(&path)?;
        let entries = index::parse_index(&mut file, &rpa_header)?;

        Ok(RpaArchive {
            path,
            header: rpa_header,
            entries,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn version(&self) -> RpaVersion {
        self.header.version
    }

    pub fn header(&self) -> &RpaHeader {
        &self.header
    }

    pub fn entries(&self) -> &HashMap<String, IndexEntry> {
        &self.entries
    }

    pub fn extract(&self, filename: &str) -> Result<Vec<u8>, RpaError> {
        let entry = self
            .entries
            .get(filename)
            .ok_or_else(|| RpaError::EntryNotFound(filename.to_string()))?;

        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(entry.offset))?;

        let mut data = vec![0u8; entry.length as usize];
        file.read_exact(&mut data)?;

        if !entry.prefix.is_empty() {
            let mut result = entry.prefix.clone();
            result.extend_from_slice(&data);
            Ok(result)
        } else {
            Ok(data)
        }
    }

    pub fn extract_to(&self, filename: &str, dest: impl AsRef<Path>) -> Result<(), RpaError> {
        let data = self.extract(filename)?;
        let dest = dest.as_ref();

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(dest, &data)?;
        Ok(())
    }

    pub fn extract_many(
        &self,
        filenames: &[&str],
        dest_dir: impl AsRef<Path>,
        mut progress: impl FnMut(usize, usize, &str),
    ) -> Result<(), RpaError> {
        let dest_dir = dest_dir.as_ref();
        let total = filenames.len();

        for (i, filename) in filenames.iter().enumerate() {
            progress(i, total, filename);
            let dest = dest_dir.join(filename);
            self.extract_to(filename, &dest)?;
        }

        progress(total, total, "");
        Ok(())
    }

    pub fn extract_all(
        &self,
        dest_dir: impl AsRef<Path>,
        progress: impl FnMut(usize, usize, &str),
    ) -> Result<(), RpaError> {
        let filenames: Vec<&str> = self.entries.keys().map(|s| s.as_str()).collect();
        self.extract_many(&filenames, dest_dir, progress)
    }
}
