use crate::error::RpaError;
use crate::header::{self, RpaHeader, RpaVersion};
use crate::index::{self, IndexEntry};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum DataSource {
    FromArchive {
        archive_path: PathBuf,
        offset: u64,
        length: u64,
        prefix: Vec<u8>,
    },
    FromFile(PathBuf),
    FromMemory(Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct PackerEntry {
    pub filename: String,
    pub source: DataSource,
}

pub struct RpaPacker {
    version: RpaVersion,
    key: u64,
    entries: Vec<PackerEntry>,
}

impl RpaPacker {
    pub fn new(version: RpaVersion) -> Self {
        let key = match version {
            RpaVersion::V2_0 => 0,
            RpaVersion::V3_0 | RpaVersion::V3_2 => 0x42424242,
        };
        RpaPacker {
            version,
            key,
            entries: Vec::new(),
        }
    }

    pub fn with_key(mut self, key: u64) -> Self {
        self.key = key;
        self
    }

    pub fn add_entry(&mut self, filename: String, source: DataSource) {
        self.entries.push(PackerEntry { filename, source });
    }

    pub fn pack(
        &self,
        output_path: impl AsRef<Path>,
        mut progress: impl FnMut(usize, usize, &str),
    ) -> Result<(), RpaError> {
        let output_path = output_path.as_ref();
        let mut output = File::create(output_path)
            .map_err(|e| RpaError::PackError(format!("Failed to create output file: {}", e)))?;

        // Write placeholder header (will be overwritten)
        let placeholder_header = header::write_header(self.version, 0, self.key);
        output.write_all(placeholder_header.as_bytes())?;

        let total = self.entries.len();
        let mut written_entries: HashMap<String, IndexEntry> = HashMap::new();

        // Write file data
        for (i, entry) in self.entries.iter().enumerate() {
            progress(i, total, &entry.filename);

            let current_offset = output.stream_position()?;
            let data = self.read_source_data(&entry.source)?;
            output.write_all(&data)?;

            written_entries.insert(
                entry.filename.clone(),
                IndexEntry {
                    offset: current_offset,
                    length: data.len() as u64,
                    prefix: Vec::new(),
                },
            );
        }

        // Record index offset
        let index_offset = output.stream_position()?;

        // Serialize and write index
        let temp_header = RpaHeader {
            version: self.version,
            index_offset,
            key: self.key,
        };
        let compressed_index = index::serialize_index(&written_entries, &temp_header)?;
        output.write_all(&compressed_index)?;

        // Seek back and write real header
        output.seek(SeekFrom::Start(0))?;
        let real_header = header::write_header(self.version, index_offset, self.key);
        output.write_all(real_header.as_bytes())?;

        progress(total, total, "");
        Ok(())
    }

    fn read_source_data(&self, source: &DataSource) -> Result<Vec<u8>, RpaError> {
        match source {
            DataSource::FromArchive {
                archive_path,
                offset,
                length,
                prefix,
            } => {
                let mut file = File::open(archive_path)?;
                file.seek(SeekFrom::Start(*offset))?;
                let mut data = vec![0u8; *length as usize];
                file.read_exact(&mut data)?;
                if !prefix.is_empty() {
                    let mut result = prefix.clone();
                    result.extend_from_slice(&data);
                    Ok(result)
                } else {
                    Ok(data)
                }
            }
            DataSource::FromFile(path) => {
                let data = std::fs::read(path)
                    .map_err(|e| RpaError::PackError(format!("Failed to read {}: {}", path.display(), e)))?;
                Ok(data)
            }
            DataSource::FromMemory(data) => Ok(data.clone()),
        }
    }
}

/// Build a packer from an existing archive with edits applied.
pub fn packer_from_archive_with_edits(
    archive: &crate::archive::RpaArchive,
    removed: &std::collections::HashSet<String>,
    replaced: &HashMap<String, PathBuf>,
    added: &HashMap<String, PathBuf>,
    text_edits: &HashMap<String, String>,
) -> RpaPacker {
    let mut packer = RpaPacker::new(archive.version()).with_key(archive.header().key);

    // Add original entries (minus removed, with replacements/edits applied)
    for (filename, entry) in archive.entries() {
        if removed.contains(filename) {
            continue;
        }

        if let Some(replacement_path) = replaced.get(filename) {
            packer.add_entry(filename.clone(), DataSource::FromFile(replacement_path.clone()));
        } else if let Some(new_text) = text_edits.get(filename) {
            packer.add_entry(
                filename.clone(),
                DataSource::FromMemory(new_text.as_bytes().to_vec()),
            );
        } else {
            packer.add_entry(
                filename.clone(),
                DataSource::FromArchive {
                    archive_path: archive.path().to_path_buf(),
                    offset: entry.offset,
                    length: entry.length,
                    prefix: entry.prefix.clone(),
                },
            );
        }
    }

    // Add new files
    for (filename, path) in added {
        packer.add_entry(filename.clone(), DataSource::FromFile(path.clone()));
    }

    packer
}
