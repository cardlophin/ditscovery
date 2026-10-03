use crate::config::{Column, Error};
use std::path::{Path, PathBuf};

pub trait Loader {
    fn load(&self) -> Result<Vec<f64>, Error>;
}

pub struct CsvLoader {
    path: PathBuf,
    column: Column,
    delimiter: u8,
    header: bool,
}

impl CsvLoader {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            column: Column::Index(0),
            delimiter: b',',
            header: true,
        }
    }

    pub fn column(mut self, column: Column) -> Self {
        self.column = column;
        self
    }

    pub fn delimiter(mut self, delimiter: u8) -> Self {
        self.delimiter = delimiter;
        self
    }

    pub fn header(mut self, header: bool) -> Self {
        self.header = header;
        self
    }

    fn data(&self, msg: impl std::fmt::Display) -> Error {
        Error::Data(format!("{}: {msg}", self.path.display()))
    }

    fn index(&self, names: &[String]) -> Result<usize, Error> {
        let available = || names.join(", ");
        match &self.column {
            Column::Name(n) if !self.header => {
                Err(self.data(format!("column '{n}' needs a header row")))
            }
            Column::Name(n) => names.iter().position(|c| c == n).ok_or_else(|| {
                self.data(format!("column '{n}' not found (available: {})", available()))
            }),
            Column::Index(i) if self.header && !names.is_empty() && *i >= names.len() => {
                Err(self
                    .data(format!("column index {i} out of range (available: {})", available())))
            }
            Column::Index(i) => Ok(*i),
        }
    }
}

fn parse(cell: &str) -> Option<f64> {
    match cell {
        "" | "NA" | "na" | "null" => Some(f64::NAN),
        _ => cell.parse().ok(),
    }
}

impl Loader for CsvLoader {
    fn load(&self) -> Result<Vec<f64>, Error> {
        let file = std::fs::File::open(&self.path)
            .map_err(|source| Error::Io { path: self.path.clone(), source })?;
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(self.header)
            .delimiter(self.delimiter)
            .from_reader(file);
        let names: Vec<String> = match self.header {
            true => rdr.headers().map_err(|e| self.data(e))?.iter().map(String::from).collect(),
            false => Vec::new(),
        };
        let idx = self.index(&names)?;
        let label = names.get(idx).cloned().unwrap_or_else(|| idx.to_string());
        let mut out = Vec::new();
        for (row, rec) in rdr.records().enumerate() {
            let rec = rec.map_err(|e| self.data(e))?;
            let cell = rec.get(idx).ok_or_else(|| {
                self.data(format!("column index {idx} out of range ({} columns)", rec.len()))
            })?;
            let cell = cell.trim();
            out.push(parse(cell).ok_or_else(|| {
                self.data(format!("column '{label}' is not numeric (row {}: '{cell}')", row + 1))
            })?);
        }
        if out.is_empty() {
            return Err(self.data("empty file (no data rows)"));
        }
        if out.iter().all(|v| v.is_nan()) {
            return Err(self.data(format!("column '{label}' has no values")));
        }
        Ok(out)
    }
}
