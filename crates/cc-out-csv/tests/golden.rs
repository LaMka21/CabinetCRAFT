//! Golden test: rozpis vzorového projektu (špecifikácia MVP, kap. 10).

use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn model() -> cc_model::Model {
    let loaded = cc_config::load(&root().join("examples/s1/project.toml")).unwrap();
    cc_generate::build(&loaded.project, &loaded.standard, &loaded.materials).unwrap()
}

#[test]
fn cutlist_matches_golden() {
    let actual = cc_out_csv::render(&model(), Default::default());
    let expected = std::fs::read_to_string(root().join("tests/golden/s1/cutlist.csv"))
        .unwrap()
        .replace("\r\n", "\n");
    assert_eq!(actual, expected);
}

#[test]
fn merge_combines_identical_parts() {
    let csv = cc_out_csv::render(
        &model(),
        cc_out_csv::CsvOptions {
            merge: true,
            decimal_comma: false,
        },
    );
    // boky majú rôzne roly (ľavý/pravý), spojky tiež → zlúči sa iba to, čo je naozaj rovnaké
    assert_eq!(csv.lines().count(), 1 + 7);

    let mut loaded = cc_config::load(&root().join("examples/s1/project.toml")).unwrap();
    loaded.project.cabinets[0].adjustable_shelves = 3;
    let model = cc_generate::build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
    let csv = cc_out_csv::render(
        &model,
        cc_out_csv::CsvOptions {
            merge: true,
            decimal_comma: true,
        },
    );
    let shelf = csv
        .lines()
        .find(|l| l.contains("adjustable_shelf"))
        .unwrap();
    assert_eq!(
        shelf,
        "S1-ASHELF-1,S1-ASHELF-2,S1-ASHELF-3;S1;adjustable_shelf;DTD_18_biela;562,0;521,5;18,0;3;none;;;ABS_2_biela;;"
    );
}
