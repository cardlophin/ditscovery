use ditscover_core::Error;
use ditscover_core::config::Column;
use ditscover_core::io::{CsvLoader, Loader};
use std::path::PathBuf;

fn file(name: &str, content: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("ditscover-io-tests");
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, content).unwrap();
    p
}

fn load(name: &str, content: &str, f: impl Fn(CsvLoader) -> CsvLoader) -> Result<Vec<f64>, Error> {
    f(CsvLoader::new(file(name, content))).load()
}

fn msg(r: Result<Vec<f64>, Error>) -> String {
    r.unwrap_err().to_string()
}

#[test]
fn column_by_name_and_index() {
    let csv = "a,b\n1,2\n3,4\n";
    assert_eq!(load("n.csv", csv, |l| l.column(Column::Name("b".into()))).unwrap(), [2.0, 4.0]);
    assert_eq!(load("i.csv", csv, |l| l.column(Column::Index(1))).unwrap(), [2.0, 4.0]);
    assert_eq!(load("d.csv", csv, |l| l).unwrap(), [1.0, 3.0]);
}

#[test]
fn tsv_and_no_header() {
    let tsv =
        load("t.tsv", "a\tb\n1\t2\n", |l| l.delimiter(b'\t').column(Column::Name("b".into())));
    assert_eq!(tsv.unwrap(), [2.0]);
    let nh = load("h.csv", "1,2\n3,4\n", |l| l.header(false).column(Column::Index(1)));
    assert_eq!(nh.unwrap(), [2.0, 4.0]);
}

#[test]
fn missing_column_lists_available() {
    let e = msg(load("m.csv", "a,b\n1,2\n", |l| l.column(Column::Name("z".into()))));
    assert!(e.contains('z') && e.contains("a, b"), "{e}");
    let e = msg(load("m2.csv", "a,b\n1,2\n", |l| l.column(Column::Index(5))));
    assert!(e.contains('5') && e.contains("a, b"), "{e}");
}

#[test]
fn bad_content_is_explicit() {
    let e = msg(load("x.csv", "a,b\nx,1\n", |l| l));
    assert!(e.contains("not numeric") && e.contains('a'), "{e}");
    let e = msg(load("e.csv", "", |l| l));
    assert!(e.contains("empty"), "{e}");
    let e = msg(load("nan.csv", "a\nNaN\nNaN\n", |l| l));
    assert!(e.contains("no values"), "{e}");
    let e =
        msg(load("sep.csv", "a,b\n1,2\n", |l| l.delimiter(b';').column(Column::Name("b".into()))));
    assert!(e.contains("not found"), "{e}");
    let e = CsvLoader::new("/nonexistent/x.csv").load().unwrap_err();
    assert!(matches!(e, Error::Io { .. }));
}

#[test]
fn missing_cells_are_nan() {
    let v = load("g.csv", "a,b\n1,\n2,NaN\n3,5\n", |l| l.column(Column::Name("b".into()))).unwrap();
    assert!(v[0].is_nan() && v[1].is_nan());
    assert_eq!(v[2], 5.0);
}
